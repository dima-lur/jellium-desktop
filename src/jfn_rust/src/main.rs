//! Process entry point. Forwards into [`jfn_rust::app::jfn_app_main`],
//! which owns the full boot/run/shutdown sequence (CEF subprocess
//! dispatch, settings load, platform install, mpv boot, browser run
//! loop, teardown).

fn main() {
    // Panic hook: route panics through tracing so they land in the same log
    // file as everything else (stderr is not captured by `just run` on Windows).
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "PANIC: {info}\n{bt}");
        eprintln!("PANIC: {info}\n{bt}");
        default_hook(info);
    }));

    let mut code = jfn_rust::app::jfn_app_main();
    if jfn_gpu_paint::device_lost() {
        // Only restart after confirmed native teardown. The recovery process
        // uses software CEF frames and never recursively restarts itself.
        if code == 0 && std::env::var_os("JELLIUM_GPU_RECOVERY").is_none() {
            let restarted = std::env::current_exe().and_then(|exe| {
                let mut command = std::process::Command::new(exe);
                command.args(std::env::args_os().skip(1));
                if !std::env::args_os().any(|arg| arg == "--disable-gpu-compositing") {
                    command.arg("--disable-gpu-compositing");
                }
                command.env("JELLIUM_GPU_RECOVERY", "1");
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
                }
                command.spawn()
            });
            if let Err(error) = restarted {
                eprintln!("GPU recovery restart failed: {error}");
                code = 1;
            }
        } else {
            eprintln!(
                "GPU device lost; automatic recovery is unavailable. Restart the application."
            );
            code = 1;
        }
    }
    std::process::exit(code);
}
