use core::fmt;
use std::collections::VecDeque;
use std::env::{args};
use std::{error::Error};
use std::{path::PathBuf};
use indexmap::IndexMap;
#[cfg(target_os = "linux")]
use libloading::Library;
#[cfg(target_os = "linux")]
use libloading::Symbol;
use shared_files::core_header::*;
#[cfg(target_os = "windows")]
use windows::{
    Win32::{
        Foundation::FreeLibrary,
        Foundation::HMODULE,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::{PCSTR, PCWSTR},
};

#[cfg(target_os = "linux")]
#[derive(Debug)]
struct Module {
    path: PathBuf,
    module: Library,
}

impl Module {
    fn start(&self, core: &CoreH, args: &mut Vec<String>) -> Result<(), ModuleError> {
        let startup_fn: Symbol<extern "C" fn(core: &CoreH, args: &mut Vec<String>)>;

        unsafe {
            match self.module.get(b"module_startup\0") {
                Ok(func) => startup_fn = func,
                Err(msg) => {
                    return Err(ModuleError::LoadError(String::from(
                        format!(
                            "Failed to find module_startup function in {:?}: {:?}",
                            self.path.file_stem().unwrap(),
                            msg
                        )
                    )));
                }
            }
        }

        startup_fn(core, args);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
struct Module {
    path: PathBuf,
    module: HMODULE,
}

#[derive(Debug, PartialEq, Eq)]
enum ModuleError {
    LoadError(String),
    UnloadError(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ModuleError::LoadError(msg) => {
                write!(f, "{}", msg)
            }
            ModuleError::UnloadError(msg) => {
                write!(f, "{}", msg)
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
fn load_module_linux(
    module_name: &String,
) -> Result<Module, ModuleError> {
    use std::{fs, path::PathBuf};

    let paths;
    let mut library_file: Option<PathBuf> = None;

    match fs::read_dir("modules") {
        Ok(data) => paths = data,
        Err(msg) => {
            if let Err(msg2) = fs::create_dir("modules") {
                return Err(ModuleError::LoadError(format!(
                    "Failed to create module folder: {:?}",
                    msg2
                )));
            }

            return Err(ModuleError::LoadError(format!(
                "Module folder (\"module\') was missing and has been created: {:?}",
                msg
            )));
        }
    }

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

        match checked_path.path().file_stem() {
            Some(file_name) => {
                if let Some(f_name) = file_name.to_str()
                && f_name.strip_prefix("lib").unwrap() == module_name {
                    library_file = Some(checked_path.path());
                    break;
                }
                continue;
            },
            None => continue,
        }
    }

    if library_file.is_none() {
        return Err(ModuleError::LoadError(format!("Module {} not found!", module_name)));
    }

    let library;

    unsafe {
        match Library::new(library_file.as_ref().unwrap()) {
            Ok(data) => library = data,
            Err(msg) => {
                return Err(ModuleError::LoadError(String::from(
                    format!("Module {} failed to load: {:?}", module_name, msg)
                )));
            }
        }
    }

    return Ok(Module { path: library_file.unwrap(), module: library });
}

#[cfg(target_os = "linux")]
fn load_modules_linux(
    core: &CoreH,
    args: &Vec<String>,
) -> Result<Vec<Module>, ModuleError> {
    use std::{fs};

    let mut library_names = Vec::new();

    let paths;

    match fs::read_dir("modules") {
        Ok(data) => paths = data,
        Err(msg) => {
            if let Err(msg2) = fs::create_dir("modules") {
                return Err(ModuleError::LoadError(format!(
                    "Failed to create module folder: {:?}",
                    msg2
                )));
            }

            return Err(ModuleError::LoadError(format!(
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
        return Err(ModuleError::LoadError(String::from("Found no modules!")));
    }

    let mut failed_modules: usize = 0;
    let mut libraries = Vec::new();

    for module_path in library_names {
        unsafe {
            let library;

            match Library::new(&module_path) {
                Ok(data) => library = data,
                Err(msg) => {
                    failed_modules += 1;
                    println!("Failed to load library!: {}", msg);
                    continue;
                }
            }

            let startup_fn: Symbol<extern "C" fn(core: &CoreH, args: &mut Vec<String>)>;

            match library.get(b"module_startup\0") {
                Ok(func) => startup_fn = func,
                Err(msg) => {
                    failed_modules += 1;
                    println!("Did not find startup function: {}", msg);
                    continue;
                }
            }

            let mut module_args = args.clone();
            startup_fn(core, &mut module_args);

            libraries.push(Module { path: module_path, module: library });
        }
    }

    if failed_modules > 0 {
        println!("Failed to load {} module(s)!", failed_modules);
    }

    return Ok(libraries);
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
fn unload_module_linux(
    core: &CoreH,
    library: Module,
) -> Result<(), ModuleError> {
    unsafe {
        let shutdown_fn: Symbol<extern "C" fn(core: &CoreH)>;

        match library.module.get(b"module_shutdown\0") {
            Ok(func) => shutdown_fn = func,
            Err(msg) => {
                return Err(ModuleError::UnloadError(String::from(
                    format!(
                        "Failed to find module_shutdown function in {:?}: {:?}",
                        library.path.file_stem().unwrap(),
                        msg
                    )
                )));
            }
        }

        shutdown_fn(core);
    }

    if let Err(msg) = library.module.close() {
        return Err(ModuleError::UnloadError(String::from(
            format!(
                "Failed to unload module {:?}: {:?}",
                library.path.file_stem().unwrap(),
                msg
            )
        )));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn unload_modules_linux(
    core: &CoreH,
    libraries: Vec<Module>,
) -> Result<(), ModuleError> {
    let mut failed_modules: usize = 0;

    for library in libraries.iter() {
        unsafe {
            let shutdown_fn: Symbol<extern "C" fn(core: &CoreH)>;

            match library.module.get(b"module_shutdown\0") {
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

    let len = libraries.len();

    for library in libraries {
        if let Err(msg) = library.module.close() {
            failed_modules += 1;
            println!("Failed to unload library {:?}: {:?}", library.path.file_stem().unwrap(), msg);
            continue;
        }
    }

    if failed_modules == len {
        return Err(ModuleError::UnloadError(String::from(
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
    let mut main_args = args().collect::<VecDeque<_>>();

    let first_arg = main_args.pop_front().unwrap();
    let mut global_args: Option<Vec<String>> = None;

    let mut seperated_args: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut last_main_arg = String::from("");

    for _i in 0..main_args.len() {
        let arg = main_args.pop_front().unwrap();

        if seperated_args.is_empty() && !arg.contains('+') {
            if global_args.is_none() {
                global_args = Some(Vec::new());
            }

            global_args.as_mut().unwrap().push(arg);
            continue;
        }

        if arg.contains('+') {
            last_main_arg = String::from(arg.strip_prefix('+').unwrap());
            seperated_args.insert(String::from(arg.to_owned().strip_prefix('+').unwrap()), Vec::new());
            continue;
        }
        
        if seperated_args.contains_key(&last_main_arg) {
            seperated_args.get_mut(&last_main_arg).unwrap().push(arg);
        }
    }

    let core = CoreH {
        ping_core_f: ping_core,
    };

    if global_args.is_some() {
        global_args.as_mut().unwrap().insert(0, first_arg.clone());

        if global_args.as_ref().unwrap().contains(&String::from("ping")) {
            ping_core();
        }

        let libraries;

        match load_modules_linux(&core, global_args.as_ref().unwrap()) {
            Ok(libs) => libraries = libs,
            Err(msg) => {
                println!("{:?}", msg);
                return;
            },
        }

        if let Err(msg) = unload_modules_linux(&core, libraries) {
            println!("{:?}", msg);
        }
    }

    for (module_name, mut args) in seperated_args {
        args.insert(0, first_arg.clone());

        if module_name == "core" {
            if args.contains(&String::from("ping")) {
                ping_core();
            }
            continue;
        }

        match load_module_linux(&module_name) {
            Ok(module) => {
                if let Err(msg) = module.start(&core, &mut args) {
                    println!("{:?}", msg);
                }
                if let Err(msg) = unload_module_linux(&core, module) {
                    println!("{:?}", msg);
                }
            },
            Err(msg) => {
                println!("{:?}", msg);
                continue;
            },
        }
    }
}
