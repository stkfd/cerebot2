//! The types in this module are shared between server and client. Typescript definitions for
//! the client are automatically generated on startup in debug builds.

use std::fs::File;
use std::io::Write;

mod commands;

#[cfg(debug_assertions)]
pub fn output_typescript_definitions() {
    use typescript_definitions::TypeScriptifyTrait;

    let mut file = File::create("web/ui/server-types.d.ts").expect("Create definitions file");
    //file.write(Test::type_script_ify().as_bytes()).unwrap();
}
