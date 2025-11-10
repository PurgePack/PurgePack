use std::{ffi::OsString, rc::Rc};

pub struct CoreH {
    pub args: Rc<Vec<OsString>>,
}
