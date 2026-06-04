mod audio;
mod cli;
mod editor;
mod effects;
mod gui_render;
mod language;
mod model;
mod sequencer;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("error: {}", error);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
