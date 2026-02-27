// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Shell completion generation for flightctl.

use std::io;

/// Supported shell types for completion generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl std::str::FromStr for Shell {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(Shell::Bash),
            "zsh" => Ok(Shell::Zsh),
            "fish" => Ok(Shell::Fish),
            "powershell" | "ps" => Ok(Shell::PowerShell),
            _ => Err(format!(
                "Unknown shell: '{}'. Valid: bash, zsh, fish, powershell",
                s
            )),
        }
    }
}

impl std::fmt::Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Shell::Bash => write!(f, "bash"),
            Shell::Zsh => write!(f, "zsh"),
            Shell::Fish => write!(f, "fish"),
            Shell::PowerShell => write!(f, "powershell"),
        }
    }
}

/// Generate a completion script for the given shell.
pub fn generate_completions(shell: Shell, writer: &mut dyn io::Write) -> io::Result<()> {
    match shell {
        Shell::Bash => write_bash_completions(writer),
        Shell::Zsh => write_zsh_completions(writer),
        Shell::Fish => write_fish_completions(writer),
        Shell::PowerShell => write_powershell_completions(writer),
    }
}

fn write_bash_completions(writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, r#"# flightctl bash completion"#)?;
    writeln!(writer, r#"_flightctl_completions() {{"#)?;
    writeln!(writer, r#"  local cur prev opts"#)?;
    writeln!(writer, r#"  COMPREPLY=()"#)?;
    writeln!(writer, r#"  cur="${{COMP_WORDS[COMP_CWORD]}}""#)?;
    writeln!(writer, r#"  prev="${{COMP_WORDS[COMP_CWORD-1]}}""#)?;
    writeln!(
        writer,
        r#"  opts="status start stop restart devices axis profile completions version""#
    )?;
    writeln!(
        writer,
        r#"  COMPREPLY=( $(compgen -W "${{opts}}" -- "${{cur}}") )"#
    )?;
    writeln!(writer, r#"  return 0"#)?;
    writeln!(writer, r#"}}"#)?;
    writeln!(writer, r#"complete -F _flightctl_completions flightctl"#)?;
    Ok(())
}

fn write_zsh_completions(writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, r#"#compdef flightctl"#)?;
    writeln!(writer, r#"_flightctl() {{"#)?;
    writeln!(writer, r#"  local -a cmds"#)?;
    writeln!(writer, r#"  cmds=("#)?;
    writeln!(writer, r#"    'status:Show service status'"#)?;
    writeln!(writer, r#"    'start:Start the service'"#)?;
    writeln!(writer, r#"    'stop:Stop the service'"#)?;
    writeln!(writer, r#"    'restart:Restart the service'"#)?;
    writeln!(writer, r#"    'devices:List connected devices'"#)?;
    writeln!(writer, r#"    'axis:Axis management commands'"#)?;
    writeln!(writer, r#"    'profile:Profile management commands'"#)?;
    writeln!(
        writer,
        r#"    'completions:Generate shell completion scripts'"#
    )?;
    writeln!(writer, r#"    'version:Show version information'"#)?;
    writeln!(writer, r#"  )"#)?;
    writeln!(writer, r#"  _describe 'command' cmds"#)?;
    writeln!(writer, r#"}}"#)?;
    writeln!(writer, r#"_flightctl"#)?;
    Ok(())
}

fn write_fish_completions(writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, r#"# flightctl fish completions"#)?;
    writeln!(writer, r#"complete -c flightctl -f"#)?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a status -d 'Show service status'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a start -d 'Start the service'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a stop -d 'Stop the service'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a restart -d 'Restart the service'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a devices -d 'List connected devices'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a axis -d 'Axis management'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a profile -d 'Profile management'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a completions -d 'Generate shell completions'"#
    )?;
    writeln!(
        writer,
        r#"complete -c flightctl -n '__fish_use_subcommand' -a version -d 'Show version'"#
    )?;
    Ok(())
}

fn write_powershell_completions(writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, r#"# flightctl PowerShell completions"#)?;
    writeln!(
        writer,
        r#"Register-ArgumentCompleter -Native -CommandName flightctl -ScriptBlock {{"#
    )?;
    writeln!(
        writer,
        r#"  param($wordToComplete, $commandAst, $cursorPosition)"#
    )?;
    writeln!(
        writer,
        r#"  $commands = @('status', 'start', 'stop', 'restart', 'devices', 'axis', 'profile', 'completions', 'version')"#
    )?;
    writeln!(
        writer,
        r#"  $commands | Where-Object {{ $_ -like "$wordToComplete*" }} | ForEach-Object {{"#
    )?;
    writeln!(
        writer,
        r#"    [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)"#
    )?;
    writeln!(writer, r#"  }}"#)?;
    writeln!(writer, r#"}}"#)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_from_str_bash() {
        assert_eq!("bash".parse::<Shell>().unwrap(), Shell::Bash);
    }

    #[test]
    fn shell_from_str_zsh() {
        assert_eq!("zsh".parse::<Shell>().unwrap(), Shell::Zsh);
    }

    #[test]
    fn shell_from_str_fish() {
        assert_eq!("fish".parse::<Shell>().unwrap(), Shell::Fish);
    }

    #[test]
    fn shell_from_str_powershell() {
        assert_eq!("powershell".parse::<Shell>().unwrap(), Shell::PowerShell);
        assert_eq!("ps".parse::<Shell>().unwrap(), Shell::PowerShell);
    }

    #[test]
    fn shell_from_str_invalid() {
        assert!("notashell".parse::<Shell>().is_err());
    }

    #[test]
    fn shell_display() {
        assert_eq!(Shell::Bash.to_string(), "bash");
        assert_eq!(Shell::Zsh.to_string(), "zsh");
        assert_eq!(Shell::Fish.to_string(), "fish");
        assert_eq!(Shell::PowerShell.to_string(), "powershell");
    }

    #[test]
    fn bash_completions_generated() {
        let mut buf = Vec::new();
        generate_completions(Shell::Bash, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("flightctl"));
        assert!(s.contains("status"));
    }

    #[test]
    fn zsh_completions_generated() {
        let mut buf = Vec::new();
        generate_completions(Shell::Zsh, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("flightctl"));
        assert!(s.contains("profile"));
    }

    #[test]
    fn fish_completions_generated() {
        let mut buf = Vec::new();
        generate_completions(Shell::Fish, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("flightctl"));
        assert!(s.contains("devices"));
    }

    #[test]
    fn powershell_completions_generated() {
        let mut buf = Vec::new();
        generate_completions(Shell::PowerShell, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("flightctl"));
        assert!(s.contains("version"));
    }

    #[test]
    fn all_shells_produce_nonempty_output() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
            let mut buf = Vec::new();
            generate_completions(shell, &mut buf).unwrap();
            assert!(!buf.is_empty(), "empty output for {shell}");
        }
    }
}
