// Created by VaynNecol on 2021/9/22.
// This module can be used in RAP
// and is also available for heap cost analysis for new Rust RFC.

// The binary is automated emitted by 'install_rap.sh' when building all components in RAP
// and the cmake file 'CMakeList.txt' lies in the root dir.
// We designed this tool to serve in Unix-like environment and currently do not support windows as host system.
// Users can also use rap_phase_llvm to select one ll-file only if they pass the path to this tool.

#include <iostream>
#include <unordered_map>
#include <unordered_set>
#include <queue>
#include <llvm/IR/BasicBlock.h>
#include <llvm/IR/Instructions.h>
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IRReader/IRReader.h>
#include <llvm/Support/SourceMgr.h>
#include <llvm/Support/ManagedStatic.h>
#include <llvm/Support/CommandLine.h>
#include <llvm/Support/FileSystem.h>
#include "test.h"

using namespace llvm;

// The Global Context in LLVM
static ManagedStatic<LLVMContext> GlobalContext;
// The Global CLI Argument for 'main' and the argument is actually the '.ll' file in rap
static cl::opt<std::string> InputFile(cl::Positional, cl::desc("<filename>.ll"), cl::Required);

// This function is using dfs to travers the graph (map) of related functions and marks all functions
// that would call function '__rust_dealloc'
void visit_call_graph(
        const std::string &name,
        std::unordered_map<std::string, std::unordered_set<std::string>> &map,
        std::unordered_set<std::string> &visit,
        std::unordered_set<std::string> &taint,
        std::vector<std::string> &path
)
{
    if (taint.find(name) != taint.end()) {
        for (auto &f : path) {
            taint.insert(f);
        }
        return;
    }

    if (visit.find(name) != visit.end()) return;

    path.push_back(name);

    for (auto &f : map[name]) {
        visit_call_graph(f, map, visit, taint, path);
    }

    path.pop_back();

    visit.insert(name);
}

// This function will print the final call graph that we interested (those functions would incur memory deallocation)
void print_call_graph(
        const std::string &name,
        std::unordered_map<std::string, std::unordered_set<std::string>> &map
)
{
    for (auto &f: map[name]) {
        outs().changeColor(raw_ostream::BLUE) << "     " << f << '\n';
        print_call_graph(f, map);
    }
}

void emit_call_graph(
        std::unordered_map<std::string, std::unordered_set<std::string>> &map,
        std::string file_name
)
{
    for (int i = 0 ; i < 3 ; ++i) {
        file_name.pop_back();
    }
    file_name.append(".rap");

    raw_ostream *out = &outs();
    std::error_code EC;
    out = new raw_fd_ostream(file_name, EC, sys::fs::CD_CreateAlways);

    for (auto &f : map) {
        out->write(f.first.c_str(), f.first.size());
        out->write('\n');
        for (auto &callee : f.second) {
            out->write("     ", 5);
            out->write(callee.c_str(), callee.size());
            out->write('\n');
        }
    }
    out->flush();
}

// The input of this binary is the path (dir) to the llvm-ir file we emitted in first phase
// and the default dir in unix-like os is '/tmp/rap-llvm-ir/*.ll' that we use rap to construct.
int main(int argc, char **argv) {

    // If no path passing to rap_phase_llvm, through an error and exit.
    if (argc == 0) {
        errs() << "Failed due to lack of input LLVM-IR file for rap_phase_llvm\n";
        exit(1);
    }

    // Instance of Diagnostic
    SMDiagnostic Err;
    // Format CLI Argument
    cl::ParseCommandLineOptions(argc, argv);
    // Read and format llvm-bc file,
    // Return the Module of LLVM
    std::unique_ptr<Module> M = parseIRFile(InputFile, Err, *GlobalContext);
    // Error Handling for Parsing LLVM-IR
    if (!M) {
        Err.print(argv[0], errs());
        return 1;
    }

    // This map indirectly map the caller to its callees (as a graph form)
    // Why we take this action is that we want to perform a simple inter-procedural analysis
    // to the drop (deallocate) function in Rust-2-LLVM-IR.
    // In this phase we grasp the function that name has "drop in place" and "ops..drop",
    // due to rust mangling the original function to a non-humankind-friendly form.
    // And then we collete these functions and construct the call graph of them.
    // Note that "__rust_dealloc" is a terminator symbol in rustc to call the real destructor of a heap instance.
    std::unordered_map<std::string, std::unordered_set<std::string>> f_map;
    // This queue including all functions that we need to perform analysis
    std::queue<Function *> f_queue;
    std::unordered_set<std::string> visit;
    std::unordered_set<std::string> taint;
    std::vector<std::string> path;

    // __rust_dealloc is a terminator that we perform inter-procedural analysis
    f_map["__rust_dealloc"];
    taint.insert("__rust_dealloc");
    visit.insert("__rust_dealloc");

    // For each function in this module we search information with keyword dealloction through its name,
    // and then add them into our queue to start constructing call graph
    for (Function &F : *M) {
        // Get the name of the caller function and filter the function that we do not care
        if ((F.getName().contains("drop_in_place")
            || F.getName().contains("core..ops..drop..Drop")
            || F.getName().contains("free")
            || F.getName().contains("dealloc")
            ))
        {
            f_queue.push(&F);
        }
    }

    // Traverse the function queue and add the callee of caller function to make call graph sound
    while (!f_queue.empty()) {
        Function *F = f_queue.front();
        f_queue.pop();

        std::string caller = F->getName().str();
        f_map[caller];

        // For each basic block of the caller function F, grasp each callee in statement and add into our map(set)
        for (BasicBlock &BB : *F) {
            for (Instruction &I : BB) {
                // Try to cast Instruction to CallInst / InvokeInst (including call and invoke) if possible
                // and CI is false if the cast failed.
                const CallInst *CI = dyn_cast<CallInst>(&I);
                const InvokeInst *II = dyn_cast<InvokeInst>(&I);

                // Get the function pointer to callee and extract the name of callee if possible
                // If our casting fails, CI / II is NULL than continue
                Function *CALLEE;
                if (CI) {
                    CALLEE = CI->getCalledFunction();
                } else if (II) {
                    CALLEE = II->getCalledFunction();
                } else {
                    continue;
                }

                // Filter the case if callee is NULL, this is due to following example
                // %0 = bitcast [3 x i64]* %_1.1 to void ({}*)**
                // %1 = getelementptr inbounds void ({}*)*, void ({}*)** %0, i64 0
                // %2 = load void ({}*)*, void ({}*)** %1, align 8, !invariant.load !2, !nonnull !2
                // call void %2({}* %_1.0)
                if (!CALLEE) {
                    continue;
                }

                std::string callee = CALLEE->getName().str();
                f_map[caller].insert(callee);

                // Filter the function that we already added to map
                if (f_map.find(callee) != f_map.end()) {
                    continue;
                }

                f_queue.push(CALLEE);
            }
        }
    }

    // Visit call graph and mark the function that will incur deallocation
    for (auto &f: f_map) {
        path.clear();
        visit_call_graph(f.first, f_map, visit, taint, path);
    }

    // Delete all functions in our map and sets to shrink the scope we search and output
    std::unordered_set<std::string> clean_map_record;
    for (auto &f: f_map) {
        if (taint.find(f.first) == taint.end()) {
            clean_map_record.insert(f.first);
            continue;
        }

        std::unordered_set<std::string> clean_set_record;
        for (auto &callee : f.second) {
            if (taint.find(callee) == taint.end()) {
                clean_set_record.insert(callee);
            }
        }

        for (auto &callee : clean_set_record) {
            f.second.erase(callee);
        }
    }

    for (auto &f: clean_map_record) {
        f_map.erase(f);
    }

    for (auto &t : taint) {
        path.clear();
        // outs().changeColor(raw_ostream::RED, true) << "taint: "<< t << '\n';
        // print_call_graph(t, f_map);
    }

    emit_call_graph(f_map, argv[1]);

    outs().changeColor(raw_ostream::BLACK, true);
}