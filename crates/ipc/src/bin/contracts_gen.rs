//! Prints the IPC contract document (JSON Schema based) to stdout.

fn main() {
    let doc = wisp_ipc::schema::contract_document();
    match serde_json::to_string_pretty(&doc) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("failed to serialize contract document: {e}");
            std::process::exit(1);
        }
    }
}
