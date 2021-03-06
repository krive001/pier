use prettytable::{cell, format, row, Table};
use snafu::{ensure, OptionExt, ResultExt};
use std::fs;
use std::{path::PathBuf, process::ExitStatus};
pub mod cli;
mod config;
pub mod error;
use config::Config;
pub mod defaults;
mod macros;
use defaults::*;
pub mod script;
use error::*;
use scrawl;
use script::Script;

// Creates a Result type that return PierError by default
pub type Result<T, E = PierError> = ::std::result::Result<T, E>;

/// Main library interface
#[derive(Debug, Default)]
pub struct Pier {
    config: Config,
    pub path: PathBuf,
    verbose: bool,
}

impl Pier {
    /// Wrapper to write the configuration to path.
    pub fn write(&self) -> Result<()> {
        self.config.write(&self.path)?;

        Ok(())
    }

    pub fn config_init(&mut self, new_path: Option<PathBuf>) -> Result<()> {
        self.path = new_path
            .unwrap_or(fallback_path().unwrap_or(xdg_config_home!("pier/config.toml").unwrap()));

        ensure!(!self.path.exists(), ConfigInitFileAlreadyExists { path: &self.path.as_path() });

	if let Some(parent_dir) = &self.path.parent() {
	    if !parent_dir.exists() {
		fs::create_dir(parent_dir).context(CreateDirectory)?;
	    }
	};

	&self.add_script(Script {
	    alias: String::from("hello-pier"),
	    command: String::from("echo Hello, Pier!"),
	    description: Some(String::from("This is an example command.")),
	    reference: None,
	    tags: None,
	});

	self.write()?;

	Ok(())
    }

    pub fn new() -> Self {
        Pier::default()
    }

    /// Create new pier directly from path.
    pub fn from_file(path: PathBuf, verbose: bool) -> Result<Self> {
        let pier = Self {
            config: Config::from(&path)?,
            verbose,
            path,
        };
        Ok(pier)
    }
    /// Create new pier from what might be a path, otherwise use the first existing default path.
    pub fn from(input_path: Option<PathBuf>, verbose: bool) -> Result<Self> {
        let path = match input_path {
            Some(path) => path,
            None => fallback_path()?,
        };

        let pier = Pier::from_file(path, verbose)?;

        Ok(pier)
    }

    /// Fetches a script that matches the alias
    pub fn fetch_script(&self, alias: &str) -> Result<&Script> {
        ensure!(!self.config.scripts.is_empty(), NoScriptsExists);

        let script = self
            .config
            .scripts
            .get(&alias.to_string())
            .context(AliasNotFound {
                alias: &alias.to_string(),
            })?;

        Ok(script)
    }

    /// Edits a script that matches the alias
    pub fn edit_script(&mut self, alias: &str) -> Result<&Script> {
        ensure!(!self.config.scripts.is_empty(), NoScriptsExists);

        let mut script =
            self.config
            .scripts
            .get_mut(&alias.to_string())
            .context(AliasNotFound {
                alias: &alias.to_string(),
            })?;

        script.command = open_editor(Some(&script.command))?;

        println!("Edited {}", &alias);

        Ok(script)
    }

    /// Removes a script that matches the alias
    pub fn remove_script(&mut self, alias: &str) -> Result<()> {
        ensure!(!self.config.scripts.is_empty(), NoScriptsExists);

        self.config
            .scripts
            .remove(&alias.to_string())
            .context(AliasNotFound {
                alias: &alias.to_string(),
            })?;

        println!("Removed {}", &alias);

        Ok(())
    }

    /// Adds a script that matches the alias
    pub fn add_script(&mut self, script: Script) -> Result<()> {
        ensure!(
            !&self.config.scripts.contains_key(&script.alias),
            AliasAlreadyExists {
                alias: script.alias
            }
        );
        println!("Added {}", &script.alias);

        self.config.scripts.insert(script.alias.to_string(), script);

        Ok(())
    }

    /// Prints only the aliases in current config file that matches tags.
    pub fn list_aliases(&self, tags: Option<Vec<String>>) -> Result<()> {
        ensure!(!self.config.scripts.is_empty(), NoScriptsExists);

        for (alias, script) in &self.config.scripts {
            match (&tags, &script.tags) {
                (Some(list_tags), Some(script_tags)) => {
                    for tag in list_tags {
                        if script_tags.contains(tag) {
                            println!("{}", alias);

                            continue;
                        }
                    }
                }
                (None, _) => {
                    println!("{}", alias);

                    continue;
                }
                _ => (),
            };
        }

        Ok(())
    }

/// Copy an alias a script that matches the alias
    pub fn copy_script(
        &mut self,
        from_alias: &str,
        new_alias: &str
    ) -> Result<()> {
        ensure!(!&self.config.scripts.contains_key(&new_alias.to_string()),
            AliasAlreadyExists {
                alias: new_alias
            }
        );

        // TODO: refactor the line below.
        let script = self
            .config
            .scripts
            .get(&from_alias.to_string())
            .context(AliasNotFound {
                alias: &from_alias.to_string(),
            })?.clone();

        println!("Copy from alias {} to new alias {}", &from_alias.to_string(), &new_alias.to_string());

        self.config.scripts.insert(new_alias.to_string(), script);

        Ok(())
    }

    /// Prints a terminal table of the scripts in current config file that matches tags.
    pub fn list_scripts(
        &self,
        tags: Option<Vec<String>>,
        cmd_full: bool,
        cmd_width: Option<usize>,
    ) -> Result<()> {
        let width = match (cmd_width, self.config.default.command_width) {
            (Some(width), _) => width,
            (None, Some(width)) => width,
            (None, None) => FALLBACK_COMMAND_DISPLAY_WIDTH,
        };
        ensure!(!self.config.scripts.is_empty(), NoScriptsExists);

        let mut table = Table::new();
        // table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_format(*format::consts::FORMAT_DEFAULT);
        table.set_titles(row!["Alias", "Tag(s)", "Description", "Command"]);

        for (alias, script) in &self.config.scripts {

            match (&tags, &script.tags, &script.description) {
                (Some(list_tags), Some(script_tags), Some(description)) => {
                    for tag in list_tags {
                        if script_tags.contains(tag) {
                            table.add_row(row![
                                &alias,
                                script_tags.join(", "),
                                description,
                                script.display_command(cmd_full, width)
                            ]);

                            continue;
                        }
                    }
                }
                (Some(list_tags), Some(script_tags), None) => {
                    for tag in list_tags {
                        if script_tags.contains(tag) {
                            table.add_row(row![
                                &alias,
                                script_tags.join(", "),
                                "",
                                script.display_command(cmd_full, width)
                            ]);

                            continue;
                        }
                    }
                }
                (None, Some(script_tags), Some(description)) => {
                    table.add_row(row![
                        &alias,
                        script_tags.join(", "),
                        description,
                        script.display_command(cmd_full, width)
                    ]);

                    continue;
                }
                (None, Some(script_tags), None) => {
                    table.add_row(row![
                        &alias,
                        script_tags.join(", "),
                        "",
                        script.display_command(cmd_full, width)
                        
                    ]);

                    continue;
                }
                (None, None, None) => {
                    table.add_row(row![&alias, "", "", script.display_command(cmd_full, width)]);

                    continue;
                }
                _ => (),
            };
        }

        table.printstd();

        Ok(())
    }

    /// Runs a script and print stdout and stderr of the command.
    pub fn run_script(&self, alias: &str, args: Vec<String>) -> Result<ExitStatus> {
        let script = self.fetch_script(alias)?;
        let interpreter = match self.config.default.interpreter {
            Some(ref interpreter) => interpreter.clone(),
            None => fallback_shell(),
        };

        if self.verbose {
            println!("Starting script \"{}\"", alias);
            println!("-------------------------");
        };

        let cmd = match script.has_shebang() {
            true => script.run_with_shebang(args)?,
            false => script.run_with_cli_interpreter(&interpreter, args)?,
        };

        let stdout = String::from_utf8_lossy(&cmd.stdout);
        let stderr = String::from_utf8_lossy(&cmd.stderr);

        if stdout.len() > 0 {
            println!("{}", stdout);
        };
        if stderr.len() > 0 {
            eprintln!("{}", stderr);
        };

        if self.verbose {
            println!("-------------------------");
            println!("Script complete");
        };

        Ok(cmd.status)
    }
}

pub fn open_editor(content: Option<&str>) -> Result<String> {
    let edited_text = scrawl::editor::new()
        .contents(match content {
            Some(txt) => txt,
            None => "",
        })
        .open()
        .context(EditorError)?;

    Ok(edited_text)
}
