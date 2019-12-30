mod public_types;

fn main() {
    if cfg!(debug_assertions) {
        public_types::output_typescript_definitions();
    };

    println!("Hello, world!");
}
