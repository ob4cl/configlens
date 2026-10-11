//! Shell completion scripts generated from the live clap command definition (spec §10 packaging).
//!
//! No `clap_complete` dependency: the generator walks the same `clap::Command` tree the parser
//! uses, so the completions cannot drift from the actual CLI surface. Generation is offline,
//! deterministic and writes nothing to disk — `configlens completions --shell <shell>` prints the
//! script to stdout for the user to source or install.

use clap::{Command, CommandFactory};

/// Supported completion shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }
}

fn root() -> Command {
    crate::cli::Cli::command()
}

/// `--long` (and `-s`) flags declared directly on a command.
fn flags(cmd: &Command) -> Vec<String> {
    let mut out = Vec::new();
    for arg in cmd.get_arguments() {
        if let Some(long) = arg.get_long() {
            out.push(format!("--{long}"));
        }
        if let Some(short) = arg.get_short() {
            out.push(format!("-{short}"));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Every option flag that appears anywhere in the command tree.
///
/// Because ConfigLens carries its shared options on each subcommand (via a flattened `CommonArgs`),
/// zsh/fish offer the union so a flag completes on any subcommand.
fn all_flags(cmd: &Command) -> Vec<String> {
    let mut out = flags(cmd);
    for sub in cmd.get_subcommands() {
        out.extend(flags(sub));
    }
    out.sort();
    out.dedup();
    out
}

/// Subcommands as `(name, about)`, sorted by name for deterministic output.
fn subcommands(cmd: &Command) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = cmd
        .get_subcommands()
        .map(|sub| {
            (
                sub.get_name().to_string(),
                sub.get_about()
                    .map(|about| about.to_string())
                    .unwrap_or_default(),
            )
        })
        .collect();
    out.sort();
    out
}

/// Render the completion script for `shell`.
pub fn script(shell: Shell) -> String {
    let cmd = root();
    match shell {
        Shell::Bash => bash(&cmd),
        Shell::Zsh => zsh(&cmd),
        Shell::Fish => fish(&cmd),
    }
}

fn bash(cmd: &Command) -> String {
    let subs = subcommands(cmd);
    let commands: Vec<String> = subs.iter().map(|(name, _)| name.clone()).collect();
    let global = flags(cmd);
    let mut out = String::new();
    out.push_str("# bash completion for configlens\n");
    out.push_str("# install:  source <(configlens completions --shell bash)\n");
    out.push_str("_configlens() {\n");
    out.push_str("  local cur cmd opts\n");
    out.push_str("  COMPREPLY=()\n");
    out.push_str("  cur=\"${COMP_WORDS[COMP_CWORD]}\"\n");
    out.push_str("  cmd=\"${COMP_WORDS[1]}\"\n");
    out.push_str(&format!("  local commands=\"{}\"\n", commands.join(" ")));
    out.push_str(&format!("  local global_opts=\"{}\"\n", global.join(" ")));
    out.push_str("  if [ \"$COMP_CWORD\" -eq 1 ]; then\n");
    out.push_str("    COMPREPLY=( $(compgen -W \"$commands $global_opts\" -- \"$cur\") )\n");
    out.push_str("    return 0\n");
    out.push_str("  fi\n");
    out.push_str("  case \"$cmd\" in\n");
    for (name, _) in &subs {
        let mut opts = cmd
            .get_subcommands()
            .find(|sub| sub.get_name() == name)
            .map(flags)
            .unwrap_or_default();
        opts.extend(global.iter().cloned());
        opts.sort();
        opts.dedup();
        out.push_str(&format!("    {name}) opts=\"{}\" ;;\n", opts.join(" ")));
    }
    out.push_str("    *) opts=\"$global_opts\" ;;\n");
    out.push_str("  esac\n");
    out.push_str("  if [[ \"$cur\" == -* ]]; then\n");
    out.push_str("    COMPREPLY=( $(compgen -W \"$opts\" -- \"$cur\") )\n");
    out.push_str("  else\n");
    out.push_str("    COMPREPLY=( $(compgen -f -- \"$cur\") )\n");
    out.push_str("  fi\n");
    out.push_str("  return 0\n");
    out.push_str("}\n");
    out.push_str("complete -F _configlens configlens\n");
    out
}

fn zsh(cmd: &Command) -> String {
    let subs = subcommands(cmd);
    let global = all_flags(cmd);
    let mut out = String::new();
    out.push_str("#compdef configlens\n");
    out.push_str("# zsh completion for configlens\n");
    out.push_str("_configlens() {\n");
    out.push_str("  local -a commands\n");
    out.push_str("  commands=(\n");
    for (name, about) in &subs {
        let about = about.replace('\'', "");
        out.push_str(&format!("    '{name}:{about}'\n"));
    }
    out.push_str("  )\n");
    out.push_str("  _arguments '1: :->cmds' '*::arg:->args'\n");
    out.push_str("  case $state in\n");
    out.push_str("    cmds) _describe 'configlens command' commands ;;\n");
    out.push_str(&format!(
        "    args) compadd -- {}\n",
        global
            .iter()
            .map(|flag| format!("'{flag}'"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    out.push_str("  esac\n");
    out.push_str("}\n");
    out.push_str("_configlens \"$@\"\n");
    out
}

fn fish(cmd: &Command) -> String {
    let subs = subcommands(cmd);
    let global = all_flags(cmd);
    let mut out = String::new();
    out.push_str("# fish completion for configlens\n");
    out.push_str("complete -c configlens -f\n");
    out.push_str("complete -c configlens -s h -l help -d 'Show help'\n");
    for (name, about) in &subs {
        let about = about.replace('\'', "");
        out.push_str(&format!(
            "complete -c configlens -n '__fish_use_subcommand' -a '{name}' -d '{about}'\n"
        ));
    }
    for flag in &global {
        if let Some(long) = flag.strip_prefix("--") {
            out.push_str(&format!("complete -c configlens -l {long}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_script_lists_commands_and_options() {
        let script = script(Shell::Bash);
        for command in [
            "clients", "scan", "explain", "doctor", "snapshot", "diff", "schema",
        ] {
            assert!(script.contains(command), "bash missing {command}");
        }
        assert!(script.contains("--client"));
        assert!(script.contains("complete -F _configlens configlens"));
    }

    #[test]
    fn zsh_script_is_a_compdef_function() {
        let script = script(Shell::Zsh);
        assert!(script.starts_with("#compdef configlens"));
        assert!(script.contains("_describe 'configlens command' commands"));
        assert!(script.contains("--paths"));
    }

    #[test]
    fn fish_script_registers_subcommands() {
        let script = script(Shell::Fish);
        assert!(script.contains("complete -c configlens"));
        assert!(script.contains("-a 'explain'"));
        assert!(script.contains("-l client"));
    }

    #[test]
    fn completion_generation_is_deterministic() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            assert_eq!(script(shell), script(shell));
        }
    }
}
