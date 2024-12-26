#!/bin/bash
#该脚本在目录下为每个Cargo项目执行相同的命令直到报错

# All arguments passed to this script are forwarded to cargo rap
# Example: batch.sh -F -M

cur=$(pwd)

# 查找并编译当前目录下的所有 Rust 项目
find support -type f -name "Cargo.toml" | while read -r cargo_file; do
  # 获取 Cargo.toml 文件所在的目录
  project_dir=$(dirname "$cargo_file")

  echo "Processing project in: $project_dir"

  # 切换到项目目录
  pushd "$project_dir" >/dev/null

  if [ $# -eq 0 ]; then
    #脚本无参数时执行cargo clean
    #Example: batch.sh
    cmd="cargo clean"
    $cmd
  else
    cmd="cargo rap $@"
    $cmd 2>&1 | tee $cur/rap.txt | ansi2txt | grep 'RAP|WARN|' && echo -e "\033[32m$project_dir pass\033[0m"
  fi

  # 返回原始目录
  popd >/dev/null

  if [ $? -ne 0 ]; then
    echo -e "Error: '$cmd' doesn't emit WARN diagnostics in $project_dir \nRAP output:"
    cat $cur/rap.txt
    exit 1
  fi

  cat $cur/rap.txt | ansi2txt | grep 'RAP|ERROR|'
  if [ $? -eq 0 ]; then
    echo -e "Error: '$cmd' contains error message in $project_dir \nRAP output:"
    cat $cur/rap.txt
    exit 1
  fi

done
