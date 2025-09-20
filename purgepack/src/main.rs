use core::fmt;
use std::env;
use std::ffi::OsString;
use std::{cell::RefCell, error::Error, rc::Rc};
use std::{collections::HashMap, path::PathBuf};

use shared_files::core_header;
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::FreeLibrary,
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{PCSTR, PCWSTR},
};

#[derive(Debug)]
pub enum ModuleError {
    FileSystemError(String),
    AllModuleLoadError(String),
    AllModuleUnloadError(String),
    ModuleUnloadError(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ModuleError::FileSystemError(msg) => write!(f, "A filesystem error occured: {}", msg),
            ModuleError::AllModuleLoadError(msg) => {
                write!(f, "All modules failed to load: {}", msg)
            }
            ModuleError::AllModuleUnloadError(msg) => {
                write!(f, "Failed to unload all modules: {}", msg)
            }
            ModuleError::ModuleUnloadError(msg) => {
                write!(f, "Failed to unload module: {}", msg)
            }
        }
    }
}

impl Error for ModuleError {}

fn print_core(s: &str) {
    println!("{}", s);
    println!("Someone called this :O");
}

#[cfg(windows)]
fn load_modules_windows(
    core_header: &core_header::CoreH,
    skip_modules: Option<Rc<Vec<OsString>>>,
) -> Result<Rc<RefCell<HashMap<PathBuf, HMODULE>>>, ModuleError> {
    use std::{collections::HashMap, fs, path::PathBuf};

    let mut dll_name: Vec<Vec<u16>> = Vec::new();
    let mut readable_dll_path = Vec::new();

    let paths;

    match fs::read_dir("modules") {
        Ok(data) => paths = data,
        Err(msg) => {
            if let Err(msg2) = fs::create_dir("modules") {
                return Err(ModuleError::FileSystemError(format!(
                    "Failed to create module folder: {:?}",
                    msg2
                )));
            }

            return Err(ModuleError::AllModuleLoadError(format!(
                "Module folder (\"module\') was missing and has been created: {:?}",
                msg
            )));
        }
    }

    let mut number_of_modules: usize = 0;

    for path in paths {
        let real_path;

        match path {
            Ok(data) => real_path = data,
            Err(_) => continue,
        }

        let file_type;

        match real_path.file_type() {
            Ok(data) => file_type = data,
            Err(_) => continue,
        }

        if !file_type.is_file() {
            continue;
        }

        #[cfg(windows)]
        match real_path.path().extension() {
            Some(data) => {
                if data.to_ascii_lowercase() != "dll" {
                    continue;
                }
            }
            None => continue,
        }

        let path = real_path.path();
        let file_name;

        match path.file_stem() {
            Some(data) => file_name = data,
            None => continue,
        }

        if skip_modules.is_some()
            && skip_modules
                .as_ref()
                .unwrap()
                .contains(&OsString::from(file_name))
        {
            println!("Skipped {:?}", real_path.path().file_stem());
            continue;
        }

        number_of_modules += 1;

        dll_name.push(
            real_path
                .path()
                .to_str()
                .unwrap()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect(),
        );
        readable_dll_path.push(real_path.path());
    }

    if number_of_modules <= 0 {
        return Err(ModuleError::FileSystemError(format!("Found no modules!")));
    }

    let mut failed_modules: usize = 0;
    let dll_table: Rc<RefCell<HashMap<PathBuf, HMODULE>>> = Rc::new(RefCell::new(HashMap::new()));

    for module in dll_name.iter().enumerate() {
        unsafe {
            let handle;

            match LoadLibraryW(PCWSTR(module.1.as_ptr())) {
                Ok(data) => {
                    handle = data;
                }
                Err(msg) => {
                    failed_modules += 1;
                    println!("Failed to load library!: {}", msg);
                    continue;
                }
            }

            if handle.is_invalid() {
                failed_modules += 1;
                println!("Failed to load library!");
                continue;
            }

            let func_name_c =
                std::ffi::CString::new("module_startup").expect("CString::new failed");
            let func_ptr = GetProcAddress(handle, PCSTR(func_name_c.as_ptr() as *const u8));

            if func_ptr.is_none() {
                failed_modules += 1;
                println!("Did not find startup function!");
                continue;
            }

            let startup_fn: extern "system" fn(core: &core_header::CoreH) =
                std::mem::transmute(func_ptr);

            startup_fn(&core_header);

            dll_table
                .borrow_mut()
                .insert(readable_dll_path[module.0].clone(), handle);
        }
    }

    if failed_modules > 0 {
        println!("Failed to load {} module(s)!", failed_modules);
    }

    return Ok(dll_table.clone());
}

#[cfg(windows)]
fn unload_modules_windows(
    dll_table: Rc<RefCell<HashMap<PathBuf, HMODULE>>>,
) -> Result<(), ModuleError> {
    let mut failed_modules: usize = 0;

    for (module_path, handle) in dll_table.borrow().iter() {
        unsafe {
            let func_name_c =
                std::ffi::CString::new("module_shutdown").expect("CString::new failed");
            let func_ptr = GetProcAddress(*handle, PCSTR(func_name_c.as_ptr() as *const u8));

            if func_ptr.is_none() {
                failed_modules += 1;
                println!("Did not find shutdown function!");
                continue;
            }

            let shutdown_fn: extern "system" fn() = std::mem::transmute(func_ptr);

            shutdown_fn();

            if let Err(msg) = FreeLibrary(*handle) {
                failed_modules += 1;
                println!("Failed to unload library {:?}: {:?}", module_path, msg);
                continue;
            }
        }
    }
    if failed_modules == dll_table.borrow().len() {
        return Err(ModuleError::AllModuleUnloadError(format!(
            "All modules failed to unload!"
        )));
    } else if failed_modules > 0 {
        println!("Failed to unload {:?} module(s)!", failed_modules)
    }
    Ok(())
}

fn main() {
    let args = Rc::new(env::args_os().collect::<Vec<_>>());
    println!("{:?}", args);

    let msg = Rc::new(RefCell::new(String::new()));

    let core_header = core_header::CoreH {
        print_fn: print_core,
        args: args.clone(),
        message_received: msg.clone(),
    };

    let modules;
    match load_modules_windows(&core_header, Some(args)) {
        Ok(data) => modules = data,
        Err(msg) => {
            println!("{:?}", msg);
            return;
        }
    }

    if let Err(msg) = unload_modules_windows(modules) {
        println!("{:?}", msg);
        return;
    }
}
