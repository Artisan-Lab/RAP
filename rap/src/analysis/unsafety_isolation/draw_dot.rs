use std::process::Command;
use std::fs::File;
use std::io::Write;

// please ensure 'dot' has been installed
pub fn render_dot_graphs(dot_graphs: Vec<String>) {
    for (index, dot) in dot_graphs.into_iter().enumerate() {
        let file_name = format!("graph{}.dot", index);
        let mut file = File::create(&file_name).expect("Unable to create file");
        file.write_all(dot.as_bytes()).expect("Unable to write data");

        Command::new("dot")
            .args(&["-Tpng", &file_name, "-o", &format!("graph{}.png", index)])
            .output()
            .expect("Failed to execute Graphviz dot command");
    }
}

