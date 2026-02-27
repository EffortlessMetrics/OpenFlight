/// Real-time weather data from the simulator.
#[derive(Debug, Clone)]
pub struct WeatherData {
    /// Wind speed in knots.
    pub wind_speed_kts: f64,
    /// Wind direction in degrees true (0–360).
    pub wind_direction_deg: f64,
    /// Turbulence intensity on a 0.0–1.0 scale.
    pub turbulence_intensity: f64,
    /// Gust multiplier (≥ 1.0).
    pub gust_factor: f64,
    /// Aircraft magnetic/true heading in degrees.
    pub aircraft_heading_deg: f64,
    /// Aircraft indicated airspeed in knots.
    pub airspeed_kts: f64,
}

/// Computed FFB forces derived from weather conditions.
#[derive(Debug, Clone, PartialEq)]
pub struct FfbForces {
    /// Lateral (roll-axis) force from crosswind, normalised to ±1.0.
    pub crosswind_force: f64,
    /// Longitudinal (pitch-axis) force from headwind buffet, normalised to ±1.0.
    pub headwind_buffet: f64,
    /// Periodic turbulence shake magnitude, 0.0–1.0.
    pub turbulence_shake: f64,
}

/// Configuration for the weather-to-FFB bridge.
#[derive(Debug, Clone)]
pub struct WeatherFfbConfig {
    /// Maximum wind speed (kts) that maps to full FFB force.
    pub max_wind_kts: f64,
    /// Scaling factor for turbulence effects.
    pub turbulence_gain: f64,
    /// Minimum airspeed (kts) below which weather forces are attenuated.
    pub min_airspeed_kts: f64,
}

impl Default for WeatherFfbConfig {
    fn default() -> Self {
        Self {
            max_wind_kts: 50.0,
            turbulence_gain: 1.0,
            min_airspeed_kts: 40.0,
        }
    }
}

/// Converts weather/wind telemetry into FFB forces.
pub struct WeatherFfbBridge {
    config: WeatherFfbConfig,
}

impl WeatherFfbBridge {
    #[must_use]
    pub fn new(config: WeatherFfbConfig) -> Self {
        Self { config }
    }

    /// Compute FFB forces from the current weather state.
    #[must_use]
    pub fn compute_forces(&self, weather: &WeatherData) -> FfbForces {
        let airspeed_factor = if self.config.min_airspeed_kts > 0.0 {
            (weather.airspeed_kts / self.config.min_airspeed_kts).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let relative_wind_deg =
            (weather.wind_direction_deg - weather.aircraft_heading_deg).to_radians();

        // Crosswind is the sine component of relative wind.
        let crosswind_raw = relative_wind_deg.sin()
            * (weather.wind_speed_kts / self.config.max_wind_kts)
            * weather.gust_factor;
        let crosswind_force = (crosswind_raw * airspeed_factor).clamp(-1.0, 1.0);

        // Headwind buffet is the absolute cosine component, always positive push.
        let headwind_raw = relative_wind_deg.cos().abs()
            * (weather.wind_speed_kts / self.config.max_wind_kts)
            * weather.gust_factor;
        let headwind_buffet = (headwind_raw * airspeed_factor).clamp(-1.0, 1.0);

        // Turbulence shake.
        let turbulence_shake =
            (weather.turbulence_intensity * self.config.turbulence_gain * airspeed_factor)
                .clamp(0.0, 1.0);

        FfbForces {
            crosswind_force,
            headwind_buffet,
            turbulence_shake,
        }
    }
}

impl Default for WeatherFfbBridge {
    fn default() -> Self {
        Self::new(WeatherFfbConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> WeatherFfbBridge {
        WeatherFfbBridge::default()
    }

    fn base_weather() -> WeatherData {
        WeatherData {
            wind_speed_kts: 20.0,
            wind_direction_deg: 270.0,
            turbulence_intensity: 0.0,
            gust_factor: 1.0,
            aircraft_heading_deg: 270.0,
            airspeed_kts: 120.0,
        }
    }

    #[test]
    fn zero_wind_produces_zero_forces() {
        let w = WeatherData {
            wind_speed_kts: 0.0,
            wind_direction_deg: 0.0,
            turbulence_intensity: 0.0,
            gust_factor: 1.0,
            aircraft_heading_deg: 0.0,
            airspeed_kts: 100.0,
        };
        let f = bridge().compute_forces(&w);
        assert!((f.crosswind_force).abs() < 1e-9);
        assert!((f.turbulence_shake).abs() < 1e-9);
    }

    #[test]
    fn pure_crosswind_90_degrees() {
        let mut w = base_weather();
        w.wind_direction_deg = 0.0; // 90° off heading 270
        w.aircraft_heading_deg = 270.0;
        let f = bridge().compute_forces(&w);
        // sin(90°) component should be significant
        assert!(f.crosswind_force.abs() > 0.1);
    }

    #[test]
    fn headwind_produces_buffet() {
        let mut w = base_weather();
        // Wind from the nose (same direction as heading)
        w.wind_direction_deg = 270.0;
        w.aircraft_heading_deg = 270.0;
        let f = bridge().compute_forces(&w);
        assert!(f.headwind_buffet > 0.0);
    }

    #[test]
    fn turbulence_scales_with_intensity() {
        let mut w = base_weather();
        w.turbulence_intensity = 0.5;
        let f1 = bridge().compute_forces(&w);

        w.turbulence_intensity = 1.0;
        let f2 = bridge().compute_forces(&w);

        assert!(f2.turbulence_shake > f1.turbulence_shake);
    }

    #[test]
    fn low_airspeed_attenuates_forces() {
        let mut w = base_weather();
        w.wind_direction_deg = 0.0;
        w.airspeed_kts = 120.0;
        let f_fast = bridge().compute_forces(&w);

        w.airspeed_kts = 10.0;
        let f_slow = bridge().compute_forces(&w);

        assert!(f_slow.crosswind_force.abs() < f_fast.crosswind_force.abs());
    }

    #[test]
    fn forces_clamped_to_bounds() {
        let w = WeatherData {
            wind_speed_kts: 200.0,
            wind_direction_deg: 0.0,
            turbulence_intensity: 5.0,
            gust_factor: 3.0,
            aircraft_heading_deg: 90.0,
            airspeed_kts: 300.0,
        };
        let f = bridge().compute_forces(&w);
        assert!((-1.0..=1.0).contains(&f.crosswind_force));
        assert!((-1.0..=1.0).contains(&f.headwind_buffet));
        assert!((0.0..=1.0).contains(&f.turbulence_shake));
    }

    #[test]
    fn gust_factor_amplifies() {
        let mut w = base_weather();
        w.wind_direction_deg = 0.0;
        w.gust_factor = 1.0;
        let f1 = bridge().compute_forces(&w);

        w.gust_factor = 2.0;
        let f2 = bridge().compute_forces(&w);

        assert!(f2.crosswind_force.abs() >= f1.crosswind_force.abs());
    }

    #[test]
    fn custom_config() {
        let cfg = WeatherFfbConfig {
            max_wind_kts: 100.0,
            turbulence_gain: 0.5,
            min_airspeed_kts: 20.0,
        };
        let b = WeatherFfbBridge::new(cfg);
        let w = base_weather();
        let f = b.compute_forces(&w);
        // With higher max_wind, normalised force should be smaller
        assert!(f.headwind_buffet < 1.0);
    }
}
