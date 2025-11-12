use core::fmt;
use std::env::{args};
use std::{error::Error};
use std::{collections::HashMap, path::PathBuf};
#[cfg(target_os = "linux")]
use libloading::Library;
#[cfg(target_os = "linux")]
use libloading::Symbol;
use shared_files::core_header;
#[cfg(target_os = "windows")]
use windows::{
    Win32::{
        Foundation::FreeLibrary,
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{PCSTR, PCWSTR},
};

#[derive(Debug, PartialEq, Eq)]
enum ModuleError {
    FileSystemError(String),
    AllModuleLoadError(String),
    AllModuleUnloadError(String),
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
            },
        }
    }
}

impl Error for ModuleError {}

#[cfg(target_os = "windows")]
fn load_modules_windows(
    core: &core_header::CoreH,
    seperated_args: &HashMap<String, Vec<String>>,
) -> Result<HashMap<PathBuf, HMODULE>, ModuleError> {
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

        match real_path.path().extension() {
            Some(data) => {
                if data.to_ascii_lowercase() != "dll" {
                    continue;
                }
            }
            None => continue,
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
    let mut dll_table: HashMap<PathBuf, HMODULE> = HashMap::new();

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

            let startup_fn: extern "C" fn(core: &core_header::CoreH, args: &mut Vec<String>) =
                std::mem::transmute(func_ptr);
            let mut module_args: Vec<String>;

            let module_name = format!("+{}", readable_dll_path[module.0].file_stem().unwrap()
                .to_str().unwrap());

            if let Some(args) = seperated_args.get(&module_name) {
                module_args = args.clone();
            }
            else {
                module_args = Vec::new();
            }

            startup_fn(&core, &mut module_args);

            dll_table.insert(readable_dll_path[module.0].clone(), handle);
        }
    }

    if failed_modules > 0 {
        println!("Failed to load {} module(s)!", failed_modules);
    }

    return Ok(dll_table.clone());
}

#[cfg(target_os = "linux")]
fn load_modules_linux(
    core: &core_header::CoreH,
    seperated_args: &HashMap<String, Vec<String>>,
) -> Result<HashMap<PathBuf, Library>, ModuleError> {
    use std::{collections::HashMap, fs, path::PathBuf};

    let mut library_names = Vec::new();

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
        let checked_path;

        match path {
            Ok(data) => checked_path = data,
            Err(_) => continue,
        }

        let file_type;

        match checked_path.file_type() {
            Ok(data) => file_type = data,
            Err(_) => continue,
        }

        if !file_type.is_file() {
            continue;
        }

        match checked_path.path().extension() {
            Some(data) => {
                if data.to_ascii_lowercase() != "so" {
                    continue;
                }
            }
            None => continue,
        }

        number_of_modules += 1;
        library_names.push(checked_path.path());
    }

    if number_of_modules <= 0 {
        return Err(ModuleError::FileSystemError(format!("Found no modules!")));
    }

    let mut failed_modules: usize = 0;
    let mut library_table: HashMap<PathBuf, Library> = HashMap::new();

    for module in library_names {
        unsafe {
            let library;

            match Library::new(&module) {
                Ok(data) => library = data,
                Err(msg) => {
                    failed_modules += 1;
                    println!("Failed to load library!: {}", msg);
                    continue;
                }
            }

            let startup_fn: Symbol<extern "C" fn(core: &core_header::CoreH, args: &mut Vec<String>)>;
            let mut module_args: Vec<String>;

            let module_name = format!("+{}", module.file_stem().unwrap().to_str().unwrap()
                .strip_prefix("lib").unwrap());

            if let Some(args) = seperated_args.get(&module_name) {
                module_args = args.clone();
            }
            else {
                module_args = Vec::new();
            }

            match library.get(b"module_startup\0") {
                Ok(func) => startup_fn = func,
                Err(msg) => {
                    failed_modules += 1;
                    println!("Did not find startup function: {}", msg);
                    continue;
                }
            }

            startup_fn(&core, &mut module_args);

            library_table.insert(module, library);
        }
    }

    if failed_modules > 0 {
        println!("Failed to load {} module(s)!", failed_modules);
    }

    return Ok(library_table);
}

#[cfg(target_os = "windows")]
fn unload_modules_windows(
    core: &core_header::CoreH,
    dll_table: HashMap<PathBuf, HMODULE>,
) -> Result<(), ModuleError> {
    let mut failed_modules: usize = 0;

    for (_module_path, handle) in dll_table.iter() {
        unsafe {
            let func_name_c =
                std::ffi::CString::new("module_shutdown").expect("CString::new failed");
            let func_ptr = GetProcAddress(*handle, PCSTR(func_name_c.as_ptr() as *const u8));

            if func_ptr.is_none() {
                failed_modules += 1;
                println!("Did not find shutdown function!");
                continue;
            }

            let shutdown_fn: extern "system" fn(core: &core_header::CoreH) =
                std::mem::transmute(func_ptr);

            shutdown_fn(core);
        }
    }

    for (module_path, handle) in dll_table.iter() {
        unsafe {
            if let Err(msg) = FreeLibrary(*handle) {
                failed_modules += 1;
                println!("Failed to unload library {:?}: {:?}", module_path, msg);
                continue;
            }
        }
    }

    if failed_modules == dll_table.len() {
        return Err(ModuleError::AllModuleUnloadError(format!(
            "All modules failed to unload!"
        )));
    } else if failed_modules > 0 {
        println!("Failed to unload {:?} module(s)!", failed_modules)
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn unload_modules_linux(
    core: &core_header::CoreH,
    mut library_table: HashMap<PathBuf, Library>,
) -> Result<(), ModuleError> {
    let mut failed_modules: usize = 0;

    for (_module_path, handle) in library_table.iter() {
        unsafe {
            let shutdown_fn: Symbol<extern "C" fn(core: &core_header::CoreH)>;

            match handle.get(b"module_shutdown\0") {
                Ok(func) => shutdown_fn = func,
                Err(msg) => {
                    failed_modules += 1;
                    println!("Did not find shutdown function: {}", msg);
                    continue;
                }
            }

            shutdown_fn(core);
        }
    }

    let len = library_table.len();

    for _i in 0..len {
        let key = library_table.keys().next().unwrap().clone();
        let handle = library_table.remove(&key).unwrap();

        if let Err(msg) = handle.close() {
            failed_modules += 1;
            println!("Failed to unload library {:?}: {:?}", key, msg);
            continue;
        }
    }

    if failed_modules == len {
        return Err(ModuleError::AllModuleUnloadError(format!(
            "All modules failed to unload!"
        )));
    } else if failed_modules > 0 {
        println!("Failed to unload {:?} module(s)!", failed_modules)
    }
    Ok(())
}

fn ping_core() {
    println!("Pinged core!");
}

fn main() {
    let args = args().collect::<Vec<_>>();
    let mut seperated_args = HashMap::new();
    let mut last_main_arg = "";

    for (i, arg) in args.iter().enumerate() {
        if i == 0 && !arg.contains('+') {
            continue;
        }

        if i == 1 && !arg.contains('+') {
            println!("Wrong argument format provided");
            println!("{arg}");
            return;
        }
        else if i == 1 {
            last_main_arg = arg;
        }

        if arg.contains('+') {
            seperated_args.insert(arg.clone(), Vec::new());
            last_main_arg = arg;
            continue;
        }
        
        if seperated_args.contains_key(last_main_arg) {
            seperated_args.get_mut(last_main_arg).unwrap().push(arg.clone());
        }
    }

    if let Some(core_args) = seperated_args.get("+core") {
        if core_args.contains(&String::from("ping")) {
            ping_core();
        }
    }

    let core_header = core_header::CoreH {
        ping_core_f: ping_core,
    };

    let modules;
    #[cfg(target_os = "windows")]
    match load_modules_windows(&core_header, &seperated_args) {
        Ok(data) => modules = data,
        Err(msg) => {
            println!("{:?}", msg);
            return;
        }
    }

    #[cfg(target_os = "linux")]
    match load_modules_linux(&core_header, &seperated_args) {
        Ok(data) => modules = data,
        Err(msg) => {
            println!("{:?}", msg);
            return;
        }
    }

    #[cfg(target_os = "windows")]
    if let Err(msg) = unload_modules_windows(&core_header, modules) {
        println!("{:?}", msg);
    }

    #[cfg(target_os = "linux")]
    if let Err(msg) = unload_modules_linux(&core_header, modules) {
        println!("{:?}", msg);
    }
}
