//! Run Artisan Command: the project's commands from the server, run in a terminal.

use crate::server;
use openidea_plugin::{Error, Invocation, PickItem, host};
use serde::Deserialize;
use serde_json::json;

/// The command the server's lens on a console command class carries: its name.
pub const LENS_COMMAND: &str = "laravel.runArtisan";
/// The interpreter the terminal runs; the user approves it as `process:php`.
const PHP: &str = "php";

#[derive(Deserialize)]
struct Argument {
    name: String,
    required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Flag {
    name: String,
    accepts_value: bool,
}

#[derive(Deserialize)]
struct Command {
    name: String,
    description: String,
    synopsis: String,
    arguments: Vec<Argument>,
    options: Vec<Flag>,
}

impl Command {
    /// `<post> [queue] --dry-run --queue=`: what can be typed after the name.
    fn help(&self) -> String {
        let arguments =
            self.arguments.iter().map(|a| if a.required { format!("<{}>", a.name) } else { format!("[{}]", a.name) });
        let options = self.options.iter().map(|o| format!("--{}{}", o.name, if o.accepts_value { "=" } else { "" }));
        arguments.chain(options).collect::<Vec<_>>().join(" ")
    }
}

/// Splits what was typed into arguments: spaces separate, quotes keep them together.
pub fn split_arguments(line: &str) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => current.push(c),
        }
    }
    if quote.is_some() {
        return Err(Error::Invalid("A quote is not closed".into()));
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

fn commands(ctx: &Invocation) -> Result<Vec<Command>, Error> {
    let commands: Option<Vec<Command>> = server::request(ctx, "artisanCommands", json!({}))?;
    commands.ok_or_else(|| Error::Invalid("This is not a Laravel project".into()))
}

/// Asks for the arguments when the command takes any, then runs it in a terminal.
fn run(command: &Command) -> Result<(), Error> {
    let mut args = vec!["artisan".to_owned(), command.name.clone()];
    if !command.arguments.is_empty() || !command.options.is_empty() {
        let prompt = format!("php artisan {}   ({})", command.name, command.help());
        let Some(typed) = host::input(&prompt, None)? else { return Ok(()) };
        args.extend(split_arguments(&typed)?);
    }
    host::open_terminal(&format!("artisan {}", command.name), PHP, &args).map_err(|e| match e {
        Error::Denied(_) => Error::Denied(
            "Running Artisan needs the permission to run php: allow `process:php` for this plugin in Settings › \
             Plugins"
                .into(),
        ),
        e => e,
    })
}

/// Run Artisan Command…: pick one of the project's commands and run it.
pub fn pick_and_run(ctx: &Invocation) -> Result<(), Error> {
    let commands = commands(ctx)?;
    let items: Vec<PickItem> = commands
        .iter()
        .map(|c| PickItem {
            key: c.name.clone(),
            label: c.synopsis.clone(),
            description: Some(c.description.clone()).filter(|d| !d.is_empty()),
        })
        .collect();
    let Some(name) = host::pick("Run Artisan Command", &items)? else { return Ok(()) };
    commands.iter().find(|c| c.name == name).map_or(Ok(()), run)
}

/// The lens on a console command class was clicked: its argument is the command's name.
pub fn lens(ctx: &Invocation, arg: Option<&str>) -> Result<(), Error> {
    let arguments = server::command_arguments(arg, LENS_COMMAND)?;
    let name = arguments.first().and_then(|a| a.as_str()).ok_or_else(|| Error::Invalid(LENS_COMMAND.into()))?;
    match commands(ctx)?.iter().find(|c| c.name == name) {
        Some(command) => run(command),
        None => Err(Error::NotFound(format!("The project has no Artisan command '{name}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock;

    const COMMANDS: &str = r#"[
        {"name":"posts:publish","description":"Publish due posts","synopsis":"posts:publish [--dry-run] <post>",
         "arguments":[{"name":"post","description":"","required":true,"array":false,"default":null}],
         "options":[{"name":"dry-run","shortcut":null,"description":"","acceptsValue":false,"array":false,"default":null},
                    {"name":"queue","shortcut":"Q","description":"","acceptsValue":true,"array":false,"default":null}],
         "source":"class","class":"App\\Console\\Commands\\PublishPosts","location":null},
        {"name":"inspire","description":"","synopsis":"inspire","arguments":[],"options":[],
         "source":"closure","class":null,"location":null}
    ]"#;

    #[test]
    fn arguments_split_on_spaces_and_keep_quoted_text_together() {
        assert_eq!(split_arguments("  12 --queue=high  ").unwrap(), ["12", "--queue=high"]);
        assert_eq!(split_arguments(r#"--title="Hello there" 'a b' """#).unwrap(), ["--title=Hello there", "a b", ""]);
        assert!(split_arguments("").unwrap().is_empty());
        assert!(split_arguments(r#"--title="open"#).is_err());
    }

    #[test]
    fn a_command_runs_in_a_terminal_with_the_arguments_typed() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/artisanCommands", COMMANDS);

        // Picked from the list; it takes arguments, so they are asked for.
        mock.answer_next_prompt(Some(&["posts:publish"]));
        mock.answer_next_prompt(Some(&["12 --dry-run"]));
        pick_and_run(&mock.invocation()).unwrap();
        // From the lens; a command without arguments runs at once.
        lens(&mock.invocation(), Some(r#"["inspire"]"#)).unwrap();

        let ran: Vec<_> = mock.terminals().into_iter().map(|t| (t.title, t.program, t.args)).collect();
        assert_eq!(
            ran,
            [
                (
                    "artisan posts:publish".to_owned(),
                    "php".to_owned(),
                    vec!["artisan".to_owned(), "posts:publish".to_owned(), "12".to_owned(), "--dry-run".to_owned()]
                ),
                ("artisan inspire".to_owned(), "php".to_owned(), vec!["artisan".to_owned(), "inspire".to_owned()]),
            ]
        );
        assert!(matches!(lens(&mock.invocation(), Some(r#"["nope"]"#)), Err(Error::NotFound(_))));
    }

    #[test]
    fn what_can_be_typed_is_shown_and_a_dismissed_prompt_runs_nothing() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/artisanCommands", COMMANDS);
        let commands = commands(&mock.invocation()).unwrap();
        assert_eq!(commands[0].help(), "<post> --dry-run --queue=");
        // The arguments prompt is dismissed (no scripted answer).
        lens(&mock.invocation(), Some(r#"["posts:publish"]"#)).unwrap();
        assert!(mock.terminals().is_empty());
    }

    #[test]
    fn a_project_that_is_not_laravel_says_so() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/artisanCommands", "null");
        assert!(matches!(pick_and_run(&mock.invocation()), Err(Error::Invalid(m)) if m.contains("not a Laravel")));
    }
}
