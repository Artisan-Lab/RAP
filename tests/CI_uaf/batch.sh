#!/bin/bash
#该脚本在目录下为每个Cargo项目执行相同的命令直到报错

if [ $# -eq 0 ]; then
    #脚本无参数时执行cargo clean
    #Example: batch.sh
    cmd="cargo clean"
else
    #脚本有参数时按照给定参数执行cargo命令
    #Example: batch.sh -uaf
    cmd="cargo rap $@"
fi

# 查找并编译当前目录下的所有 Rust 项目
find . -type f -name "Cargo.toml" | while read -r cargo_file; do
    # 获取 Cargo.toml 文件所在的目录
    project_dir=$(dirname "$cargo_file")
    
    echo "Processing project in: $project_dir"
    
    # 切换到项目目录
    pushd "$project_dir" > /dev/null
    
    $cmd
    if [ $? -ne 0 ]; then
        # 如果命令失败，打印错误信息并退出循环
        echo "Error: '$cmd' failed in $project_dir"
        popd > /dev/null
        exit 1
    fi
    
    # 返回原始目录
    popd > /dev/null
done

