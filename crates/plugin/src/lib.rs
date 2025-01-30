use nvim_oxi::{
    api::{
        self,
        opts::CreateCommandOpts,
        types::{CommandArgs, CommandNArgs, LogLevel},
    },
    plugin, print, Dictionary, Function, Object,
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

    print!("{alert}");
}

fn cmd(args: CommandArgs) -> nvim_oxi::Result<()> {
    match args.fargs[0].as_str() {
        "start" => start(args.fargs.get(1)),
        cmd => {
            error(format!("Unknown command \"{cmd}\""))?;
        }
    }

    Ok(())
}

fn error(e: impl Into<String>) -> nvim_oxi::Result<Object> {
    Ok(api::notify(&e.into(), LogLevel::Error, &Dictionary::new())?)
}
