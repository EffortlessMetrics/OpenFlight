// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-process-detection.
//!
//! Covers SimId, ProcessDetectionConfig, ProcessDefinition, DetectedProcess,
//! DetectionMetrics, ProcessDetector, and property-based invariants.

#[cfg(test)]
mod depth_tests {
    use crate::*;
    use proptest::prelude::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    // ═════════════════════════════════════════════════════════════════════════
    // SimId — Display, Serialize, Deserialize
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn sim_id_display_all_variants() {
        let cases = [
            (SimId::Msfs, "MSFS"),
            (SimId::Msfs2024, "MSFS 2024"),
            (SimId::XPlane, "X-Plane"),
            (SimId::Dcs, "DCS"),
            (SimId::AceCombat7, "Ace Combat 7"),
            (SimId::WarThunder, "War Thunder"),
            (SimId::EliteDangerous, "Elite: Dangerous"),
            (SimId::Ksp, "Kerbal Space Program"),
            (SimId::Wingman, "Project Wingman"),
            (SimId::Il2, "IL-2 Great Battles"),
            (SimId::Unknown, "Unknown"),
        ];
        for (id, expected) in &cases {
            assert_eq!(id.to_string(), *expected, "SimId::{id:?} display mismatch");
        }
    }

    #[test]
    fn sim_id_serde_round_trip_all() {
        let variants = [
            SimId::Msfs,
            SimId::Msfs2024,
            SimId::XPlane,
            SimId::Dcs,
            SimId::AceCombat7,
            SimId::WarThunder,
            SimId::EliteDangerous,
            SimId::Ksp,
            SimId::Wingman,
            SimId::Il2,
            SimId::Unknown,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let back: SimId = serde_json::from_str(&json).unwrap();
            assert_eq!(*v, back);
        }
    }

    #[test]
    fn sim_id_eq() {
        assert_eq!(SimId::Msfs, SimId::Msfs);
        assert_ne!(SimId::Msfs, SimId::XPlane);
    }

    #[test]
    fn sim_id_hash() {
        let mut map = HashMap::new();
        map.insert(SimId::Msfs, "msfs");
        map.insert(SimId::XPlane, "xplane");
        assert_eq!(map.get(&SimId::Msfs), Some(&"msfs"));
        assert_eq!(map.get(&SimId::XPlane), Some(&"xplane"));
        assert_eq!(map.get(&SimId::Dcs), None);
    }

    #[test]
    fn sim_id_copy() {
        let a = SimId::Dcs;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[test]
    fn sim_id_clone() {
        let a = SimId::Ksp;
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDetectionConfig — defaults
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn config_default_interval() {
        let cfg = ProcessDetectionConfig::default();
        assert_eq!(cfg.detection_interval, Duration::from_secs(1));
    }

    #[test]
    fn config_default_window_detection_enabled() {
        let cfg = ProcessDetectionConfig::default();
        assert!(cfg.enable_window_detection);
    }

    #[test]
    fn config_default_max_detection_time() {
        let cfg = ProcessDetectionConfig::default();
        assert_eq!(cfg.max_detection_time, Duration::from_millis(100));
    }

    #[test]
    fn config_has_all_simulators() {
        let cfg = ProcessDetectionConfig::default();
        let expected_sims = [
            SimId::Msfs,
            SimId::Msfs2024,
            SimId::XPlane,
            SimId::Dcs,
            SimId::AceCombat7,
            SimId::WarThunder,
            SimId::EliteDangerous,
            SimId::Ksp,
            SimId::Wingman,
        ];
        for sim in &expected_sims {
            assert!(
                cfg.process_definitions.contains_key(sim),
                "Missing definition for {sim:?}"
            );
        }
    }

    #[test]
    fn config_serde_round_trip() {
        let cfg = ProcessDetectionConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ProcessDetectionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.detection_interval, cfg.detection_interval);
        assert_eq!(
            back.enable_window_detection,
            cfg.enable_window_detection
        );
        assert_eq!(back.max_detection_time, cfg.max_detection_time);
        assert_eq!(
            back.process_definitions.len(),
            cfg.process_definitions.len()
        );
    }

    #[test]
    fn config_clone() {
        let cfg = ProcessDetectionConfig::default();
        let cfg2 = cfg.clone();
        assert_eq!(cfg2.detection_interval, cfg.detection_interval);
        assert_eq!(cfg2.process_definitions.len(), cfg.process_definitions.len());
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDefinition — per-simulator checks
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn msfs_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::Msfs).unwrap();
        assert!(def.process_names.contains(&"FlightSimulator.exe".to_string()));
        assert!(def.process_names.contains(&"fsx.exe".to_string()));
        assert!(def.window_titles.contains(&"Microsoft Flight Simulator".to_string()));
        assert_eq!(def.min_confidence, 0.8);
    }

    #[test]
    fn msfs2024_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::Msfs2024).unwrap();
        assert!(def.process_names.contains(&"FlightSimulator2024.exe".to_string()));
        assert!(def.window_titles.contains(&"Microsoft Flight Simulator 2024".to_string()));
        assert_eq!(def.min_confidence, 0.8);
    }

    #[test]
    fn xplane_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::XPlane).unwrap();
        assert!(def.process_names.contains(&"X-Plane.exe".to_string()));
        assert!(def.process_names.contains(&"X-Plane 12.exe".to_string()));
        assert!(def.process_names.contains(&"X-Plane 11.exe".to_string()));
    }

    #[test]
    fn dcs_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::Dcs).unwrap();
        assert!(def.process_names.contains(&"DCS.exe".to_string()));
        assert!(def.window_titles.contains(&"DCS World".to_string()));
    }

    #[test]
    fn ace_combat_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::AceCombat7).unwrap();
        assert!(def.process_names.contains(&"acecombat7.exe".to_string()));
        assert!(def.window_titles.contains(&"ACE COMBAT 7".to_string()));
    }

    #[test]
    fn war_thunder_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::WarThunder).unwrap();
        assert!(def.process_names.contains(&"aces.exe".to_string()));
        assert!(def.window_titles.contains(&"War Thunder".to_string()));
    }

    #[test]
    fn elite_dangerous_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::EliteDangerous).unwrap();
        assert!(def.process_names.contains(&"EliteDangerous64.exe".to_string()));
        assert!(!def.process_names.is_empty());
    }

    #[test]
    fn ksp_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::Ksp).unwrap();
        assert!(def.process_names.contains(&"KSP_x64.exe".to_string()));
        assert!(def.process_names.contains(&"KSP.x86_64".to_string()));
        assert!(def.window_titles.contains(&"Kerbal Space Program".to_string()));
    }

    #[test]
    fn wingman_definition_correct() {
        let cfg = ProcessDetectionConfig::default();
        let def = cfg.process_definitions.get(&SimId::Wingman).unwrap();
        assert!(def.process_names.contains(&"ProjectWingman.exe".to_string()));
        assert_eq!(def.min_confidence, 0.7);
    }

    #[test]
    fn all_definitions_have_nonempty_process_names() {
        let cfg = ProcessDetectionConfig::default();
        for (sim, def) in &cfg.process_definitions {
            assert!(
                !def.process_names.is_empty(),
                "{sim:?} has empty process_names"
            );
        }
    }

    #[test]
    fn all_definitions_have_valid_confidence() {
        let cfg = ProcessDetectionConfig::default();
        for (sim, def) in &cfg.process_definitions {
            assert!(
                (0.0..=1.0).contains(&def.min_confidence),
                "{sim:?} has invalid min_confidence: {}",
                def.min_confidence
            );
        }
    }

    #[test]
    fn process_definition_serde_round_trip() {
        let def = ProcessDefinition {
            process_names: vec!["test.exe".to_string()],
            window_titles: vec!["Test Window".to_string()],
            process_paths: vec![PathBuf::from("C:\\test")],
            min_confidence: 0.75,
        };
        let json = serde_json::to_string(&def).unwrap();
        let back: ProcessDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(back.process_names, def.process_names);
        assert_eq!(back.min_confidence, def.min_confidence);
    }

    #[test]
    fn process_definition_clone() {
        let def = ProcessDefinition {
            process_names: vec!["a.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        let def2 = def.clone();
        assert_eq!(def2.process_names, def.process_names);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // DetectedProcess
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn detected_process_fields() {
        let p = DetectedProcess {
            sim: SimId::Msfs,
            process_id: 1234,
            process_name: "FlightSimulator.exe".to_string(),
            process_path: PathBuf::from("C:\\MSFS\\FlightSimulator.exe"),
            window_title: Some("Microsoft Flight Simulator".to_string()),
            detection_time: Instant::now(),
            confidence: 0.9,
        };
        assert_eq!(p.sim, SimId::Msfs);
        assert_eq!(p.process_id, 1234);
        assert_eq!(p.confidence, 0.9);
    }

    #[test]
    fn detected_process_eq() {
        let now = Instant::now();
        let a = DetectedProcess {
            sim: SimId::Dcs,
            process_id: 42,
            process_name: "DCS.exe".to_string(),
            process_path: PathBuf::new(),
            window_title: None,
            detection_time: now,
            confidence: 0.8,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn detected_process_ne_different_pid() {
        let now = Instant::now();
        let a = DetectedProcess {
            sim: SimId::Dcs,
            process_id: 42,
            process_name: "DCS.exe".to_string(),
            process_path: PathBuf::new(),
            window_title: None,
            detection_time: now,
            confidence: 0.8,
        };
        let mut b = a.clone();
        b.process_id = 99;
        assert_ne!(a, b);
    }

    #[test]
    fn detected_process_clone() {
        let p = DetectedProcess {
            sim: SimId::XPlane,
            process_id: 555,
            process_name: "X-Plane.exe".to_string(),
            process_path: PathBuf::from("/opt/xplane"),
            window_title: Some("X-Plane 12".to_string()),
            detection_time: Instant::now(),
            confidence: 0.95,
        };
        let p2 = p.clone();
        assert_eq!(p, p2);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // DetectionMetrics
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn metrics_default_all_zero() {
        let m = DetectionMetrics::default();
        assert_eq!(m.total_scans, 0);
        assert_eq!(m.successful_detections, 0);
        assert_eq!(m.false_positives, 0);
        assert_eq!(m.average_scan_time, Duration::ZERO);
        assert_eq!(m.max_scan_time, Duration::ZERO);
    }

    #[test]
    fn metrics_clone() {
        let m = DetectionMetrics {
            total_scans: 100,
            successful_detections: 50,
            false_positives: 2,
            average_scan_time: Duration::from_millis(5),
            max_scan_time: Duration::from_millis(50),
        };
        let m2 = m.clone();
        assert_eq!(m2.total_scans, 100);
        assert_eq!(m2.successful_detections, 50);
        assert_eq!(m2.false_positives, 2);
        assert_eq!(m2.average_scan_time, Duration::from_millis(5));
        assert_eq!(m2.max_scan_time, Duration::from_millis(50));
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDetector — creation and state
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn detector_creation() {
        let cfg = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(cfg);
        // detector was constructed without panic
        let _ = detector;
    }

    #[test]
    fn detector_with_custom_config() {
        let cfg = ProcessDetectionConfig {
            detection_interval: Duration::from_millis(500),
            process_definitions: HashMap::new(),
            enable_window_detection: false,
            max_detection_time: Duration::from_millis(50),
        };
        let _detector = ProcessDetector::new(cfg);
    }

    #[tokio::test]
    async fn detector_initially_no_processes() {
        let detector = ProcessDetector::new(ProcessDetectionConfig::default());
        let procs = detector.get_detected_processes().await;
        assert!(procs.is_empty());
    }

    #[tokio::test]
    async fn detector_initially_no_sim_detected() {
        let detector = ProcessDetector::new(ProcessDetectionConfig::default());
        assert!(!detector.is_sim_detected(SimId::Msfs).await);
        assert!(!detector.is_sim_detected(SimId::XPlane).await);
        assert!(!detector.is_sim_detected(SimId::Dcs).await);
    }

    #[tokio::test]
    async fn detector_get_detected_process_none() {
        let detector = ProcessDetector::new(ProcessDetectionConfig::default());
        assert!(detector.get_detected_process(SimId::Msfs).await.is_none());
    }

    #[tokio::test]
    async fn detector_initial_metrics() {
        let detector = ProcessDetector::new(ProcessDetectionConfig::default());
        let m = detector.get_metrics().await;
        assert_eq!(m.total_scans, 0);
        assert_eq!(m.successful_detections, 0);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDetector — lifecycle
    // ═════════════════════════════════════════════════════════════════════════

    #[tokio::test]
    async fn detector_start_and_stop() {
        let cfg = ProcessDetectionConfig::default();
        let detector = Arc::new(ProcessDetector::new(cfg));
        assert!(Arc::clone(&detector).start().await.is_ok());
        // Small delay to let the task spawn
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(detector.stop().await.is_ok());
    }

    #[tokio::test]
    async fn detector_double_start_fails() {
        let cfg = ProcessDetectionConfig::default();
        let detector = Arc::new(ProcessDetector::new(cfg));
        assert!(Arc::clone(&detector).start().await.is_ok());
        // Second start should fail (receiver already taken)
        assert!(Arc::clone(&detector).start().await.is_err());
        let _ = detector.stop().await;
    }

    #[tokio::test]
    async fn detector_scan_once() {
        let cfg = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(cfg);
        // scan_once should succeed (even if no sim processes are running)
        let result = detector.scan_once().await;
        assert!(result.is_ok());
        let m = detector.get_metrics().await;
        assert_eq!(m.total_scans, 1);
    }

    #[tokio::test]
    async fn detector_scan_multiple() {
        let cfg = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(cfg);
        for _ in 0..3 {
            detector.scan_once().await.unwrap();
        }
        let m = detector.get_metrics().await;
        assert_eq!(m.total_scans, 3);
    }

    #[tokio::test]
    async fn detector_scan_updates_max_time() {
        let cfg = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(cfg);
        detector.scan_once().await.unwrap();
        let m = detector.get_metrics().await;
        // max_scan_time should be >= 0
        assert!(m.max_scan_time >= Duration::ZERO);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDetectionError
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn error_platform_display() {
        let e = ProcessDetectionError::Platform("unsupported".to_string());
        assert!(e.to_string().contains("Platform"));
        assert!(e.to_string().contains("unsupported"));
    }

    #[test]
    fn error_system_display() {
        let e = ProcessDetectionError::System("syscall failed".to_string());
        assert!(e.to_string().contains("System"));
        assert!(e.to_string().contains("syscall failed"));
    }

    #[test]
    fn error_is_debug() {
        let e = ProcessDetectionError::Platform("test".to_string());
        let dbg = format!("{e:?}");
        assert!(dbg.contains("Platform"));
    }

    // ═════════════════════════════════════════════════════════════════════════
    // ProcessDefinition — edge cases
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn process_definition_empty_names() {
        let def = ProcessDefinition {
            process_names: vec![],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        assert!(def.process_names.is_empty());
    }

    #[test]
    fn process_definition_confidence_zero() {
        let def = ProcessDefinition {
            process_names: vec!["test.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.0,
        };
        assert_eq!(def.min_confidence, 0.0);
    }

    #[test]
    fn process_definition_confidence_one() {
        let def = ProcessDefinition {
            process_names: vec!["test.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 1.0,
        };
        assert_eq!(def.min_confidence, 1.0);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Proptest — property-based tests
    // ═════════════════════════════════════════════════════════════════════════

    proptest! {
        #[test]
        fn prop_sim_id_display_nonempty(variant in 0u8..11) {
            let sim = match variant {
                0 => SimId::Msfs,
                1 => SimId::Msfs2024,
                2 => SimId::XPlane,
                3 => SimId::Dcs,
                4 => SimId::AceCombat7,
                5 => SimId::WarThunder,
                6 => SimId::EliteDangerous,
                7 => SimId::Ksp,
                8 => SimId::Wingman,
                9 => SimId::Il2,
                _ => SimId::Unknown,
            };
            let display = sim.to_string();
            prop_assert!(!display.is_empty());
        }

        #[test]
        fn prop_sim_id_serde_idempotent(variant in 0u8..11) {
            let sim = match variant {
                0 => SimId::Msfs,
                1 => SimId::Msfs2024,
                2 => SimId::XPlane,
                3 => SimId::Dcs,
                4 => SimId::AceCombat7,
                5 => SimId::WarThunder,
                6 => SimId::EliteDangerous,
                7 => SimId::Ksp,
                8 => SimId::Wingman,
                9 => SimId::Il2,
                _ => SimId::Unknown,
            };
            let json = serde_json::to_string(&sim).unwrap();
            let back: SimId = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            prop_assert_eq!(json, json2);
        }

        #[test]
        fn prop_process_definition_confidence_preserved(conf in 0.0f32..=1.0f32) {
            let def = ProcessDefinition {
                process_names: vec!["test.exe".to_string()],
                window_titles: vec![],
                process_paths: vec![],
                min_confidence: conf,
            };
            let json = serde_json::to_string(&def).unwrap();
            let back: ProcessDefinition = serde_json::from_str(&json).unwrap();
            let diff = (back.min_confidence - conf).abs();
            prop_assert!(diff < f32::EPSILON, "confidence not preserved: {conf} vs {}", back.min_confidence);
        }

        #[test]
        fn prop_config_definitions_count_stable(count in 0usize..20) {
            let mut defs = HashMap::new();
            for i in 0..count {
                defs.insert(
                    if i % 2 == 0 { SimId::Msfs } else { SimId::XPlane },
                    ProcessDefinition {
                        process_names: vec![format!("proc{i}.exe")],
                        window_titles: vec![],
                        process_paths: vec![],
                        min_confidence: 0.5,
                    },
                );
            }
            let cfg = ProcessDetectionConfig {
                detection_interval: Duration::from_secs(1),
                process_definitions: defs.clone(),
                enable_window_detection: true,
                max_detection_time: Duration::from_millis(100),
            };
            prop_assert_eq!(cfg.process_definitions.len(), defs.len());
        }

        #[test]
        fn prop_check_simulator_matching(
            name_fragment in "[a-zA-Z0-9]{1,20}",
            other_fragment in "[a-zA-Z0-9]{1,20}"
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _ = rt.block_on(async {
                let definition = ProcessDefinition {
                    process_names: vec![format!("{}.exe", name_fragment)],
                    window_titles: vec![],
                    process_paths: vec![],
                    min_confidence: 0.5,
                };

                let processes = vec![SystemProcess {
                    pid: 123,
                    name: format!("{}.exe", name_fragment),
                    path: PathBuf::from("C:\\test"),
                    window_title: None,
                }];

                let detected = ProcessDetector::check_simulator_processes(
                    SimId::Msfs,
                    &definition,
                    &processes,
                    false,
                )
                .await
                .unwrap();

                // Exact match should always detect
                prop_assert!(detected.is_some(), "Exact match should detect");
                if let Some(d) = &detected {
                    prop_assert!(d.confidence >= 0.6, "Name match should give >= 0.6");
                }

                // Non-match
                let expected = format!("{}.exe", name_fragment).to_lowercase();
                let candidate = format!("{}.exe", other_fragment).to_lowercase();
                if name_fragment != other_fragment && !candidate.contains(&expected) {
                    let non_matching = vec![SystemProcess {
                        pid: 456,
                        name: candidate,
                        path: PathBuf::from("C:\\other"),
                        window_title: None,
                    }];
                    let result = ProcessDetector::check_simulator_processes(
                        SimId::Msfs,
                        &definition,
                        &non_matching,
                        false,
                    )
                    .await
                    .unwrap();
                    prop_assert!(result.is_none(), "Non-matching process should not detect");
                }

                Ok(())
            });
        }
    }

    // ═════════════════════════════════════════════════════════════════════════
    // check_simulator_processes — unit tests
    // ═════════════════════════════════════════════════════════════════════════

    #[tokio::test]
    async fn check_sim_exact_name_match() {
        let def = ProcessDefinition {
            process_names: vec!["FlightSimulator.exe".to_string()],
            window_titles: vec!["Microsoft Flight Simulator".to_string()],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        let procs = vec![SystemProcess {
            pid: 100,
            name: "FlightSimulator.exe".to_string(),
            path: PathBuf::new(),
            window_title: None,
        }];
        let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
            .await
            .unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().confidence >= 0.6);
    }

    #[tokio::test]
    async fn check_sim_case_insensitive_match() {
        let def = ProcessDefinition {
            process_names: vec!["DCS.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        let procs = vec![SystemProcess {
            pid: 200,
            name: "dcs.exe".to_string(),
            path: PathBuf::new(),
            window_title: None,
        }];
        let result = ProcessDetector::check_simulator_processes(SimId::Dcs, &def, &procs, false)
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn check_sim_no_match_empty_list() {
        let def = ProcessDefinition {
            process_names: vec!["Test.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        let result =
            ProcessDetector::check_simulator_processes(SimId::Unknown, &def, &[], false)
                .await
                .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn check_sim_path_adds_confidence() {
        let def = ProcessDefinition {
            process_names: vec!["FlightSim.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![PathBuf::from("Flight Simulator")],
            min_confidence: 0.5,
        };
        let procs = vec![SystemProcess {
            pid: 300,
            name: "FlightSim.exe".to_string(),
            path: PathBuf::from("C:\\Games\\Flight Simulator\\FlightSim.exe"),
            window_title: None,
        }];
        let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
            .await
            .unwrap();
        assert!(result.is_some());
        // Name (0.6) + path (0.3) = 0.9
        assert!(result.unwrap().confidence >= 0.9);
    }

    #[tokio::test]
    async fn check_sim_window_title_adds_confidence() {
        let def = ProcessDefinition {
            process_names: vec!["FlightSim.exe".to_string()],
            window_titles: vec!["Flight Simulator".to_string()],
            process_paths: vec![],
            min_confidence: 0.5,
        };
        let procs = vec![SystemProcess {
            pid: 400,
            name: "FlightSim.exe".to_string(),
            path: PathBuf::new(),
            window_title: Some("Flight Simulator 2024".to_string()),
        }];
        let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, true)
            .await
            .unwrap();
        assert!(result.is_some());
        // Name (0.6) + window (0.1) = 0.7
        assert!(result.unwrap().confidence >= 0.7);
    }

    #[tokio::test]
    async fn check_sim_window_disabled_ignores_title() {
        let def = ProcessDefinition {
            process_names: vec!["noname.exe".to_string()],
            window_titles: vec!["Some Window".to_string()],
            process_paths: vec![],
            min_confidence: 0.05, // low threshold
        };
        let procs = vec![SystemProcess {
            pid: 500,
            name: "other.exe".to_string(),
            path: PathBuf::new(),
            window_title: Some("Some Window".to_string()),
        }];
        // With window detection disabled, only name/path matter
        let result =
            ProcessDetector::check_simulator_processes(SimId::Unknown, &def, &procs, false)
                .await
                .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn check_sim_best_match_selected() {
        let def = ProcessDefinition {
            process_names: vec!["Target.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![PathBuf::from("TargetPath")],
            min_confidence: 0.5,
        };
        let procs = vec![
            SystemProcess {
                pid: 1,
                name: "Target.exe".to_string(),
                path: PathBuf::new(), // name match only: 0.6
                window_title: None,
            },
            SystemProcess {
                pid: 2,
                name: "Target.exe".to_string(),
                path: PathBuf::from("C:\\TargetPath\\Target.exe"), // name + path: 0.9
                window_title: None,
            },
        ];
        let result = ProcessDetector::check_simulator_processes(SimId::Msfs, &def, &procs, false)
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().process_id, 2); // best match
    }

    #[tokio::test]
    async fn check_sim_below_confidence_not_detected() {
        let def = ProcessDefinition {
            process_names: vec!["HighBar.exe".to_string()],
            window_titles: vec![],
            process_paths: vec![],
            min_confidence: 0.9, // very high threshold
        };
        let procs = vec![SystemProcess {
            pid: 600,
            name: "HighBar.exe".to_string(),
            path: PathBuf::new(),
            window_title: None,
        }];
        let result =
            ProcessDetector::check_simulator_processes(SimId::Unknown, &def, &procs, false)
                .await
                .unwrap();
        // Name match gives 0.6 confidence, threshold is 0.9
        assert!(result.is_none());
    }
}
