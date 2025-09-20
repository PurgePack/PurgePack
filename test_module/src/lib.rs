use shared_files::core_header::{self, print_core};

#[cfg(target_os = "windows")]
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW},
    core::PWSTR,
};

#[unsafe(no_mangle)]
extern "system" fn module_startup(core: &core_header::CoreH) {
    print_core(core, "Hello from test module!");
    core.message_received.borrow_mut().clear();
    core.message_received.borrow_mut().push_str("Hey there :)");
    println!("Hello anyways!");

    unsafe {
        let title = "Cool Title";
        let message = "This message is from test module!";

        let title_utf16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let message_utf16: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

        MessageBoxW(
            None,
            PWSTR(message_utf16.as_ptr() as *mut u16),
            PWSTR(title_utf16.as_ptr() as *mut u16),
            MB_OK,
        );
    }
}

#[unsafe(no_mangle)]
extern "system" fn module_shutdown(core: &mut core_header::CoreH, exiting: bool) {
    unsafe {
        let title = "Shutting down!";
        let message = "This message is from test module!";

        let title_utf16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let message_utf16: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

        MessageBoxW(
            None,
            PWSTR(message_utf16.as_ptr() as *mut u16),
            PWSTR(title_utf16.as_ptr() as *mut u16),
            MB_OK,
        );
    }
    println!("Goodbye world!");
}
