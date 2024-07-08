#!/bin/bash

# 查找并编译当前目录下的所有 Rust 项目
find . -type f -name "Cargo.toml" | while read -r cargo_file; do
    # 获取 Cargo.toml 文件所在的目录
    project_dir=$(dirname "$cargo_file")
    
    echo "Processing project in: $project_dir"
    
    # 切换到项目目录
    pushd "$project_dir" > /dev/null
    
    cargo clean
    
    # 返回原始目录
    popd > /dev/null
done

