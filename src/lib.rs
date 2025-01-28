use nvim_oxi::{
    Dictionary, Function,
    api::{
        self,
        opts::CreateCommandOpts,
        types::{CommandArgs, CommandNArgs, LogLevel},
    },
    plugin, print,
};

#[plugin]
fn neoshare() -> nvim_oxi::Result<Dictionary> {
    api::create_user_command(
        "Neoshare",
        cmd,
        &CreateCommandOpts::builder()
            .desc("Starts a neoshare session")
            .nargs(CommandNArgs::OneOrMore)
            .build(),
    )?;

    Ok(Dictionary::from_iter([(
        "start",
        Function::from(|path: Option<String>| start(path)),
    )]))
}

fn start(path: Option<impl Into<String>>) {
    let mut alert = String::from("Started session");

    if let Some(args) = path {
        alert.push_str(&format!(" at \"{}\"", args.into()));
    };

    alert.push('.');

    print!("{alert}")
}

fn cmd(args: CommandArgs) {
    match args.fargs[0].as_str() {
        "start" => start(args.fargs.get(1)),
        cmd => {
            let _ = api::notify(
                &format!("Unknown command \"{cmd}\""),
                LogLevel::Error,
                &Dictionary::new(),
            );
        }
    }
}
