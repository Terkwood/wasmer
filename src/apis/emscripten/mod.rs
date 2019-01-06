use crate::runtime::module::ModuleEnvironment;
use crate::runtime::memory::LinearMemory;
use crate::runtime::types::{ElementType, FuncSig, Table, Type, Value};
use crate::runtime::{Import, Imports, Instance, Module};
/// NOTE: TODO: These emscripten api implementation only support wasm32 for now because they assume offsets are u32
use byteorder::{ByteOrder, LittleEndian};
use libc::c_int;
use std::cell::UnsafeCell;
use std::mem;
use std::sync::Arc;

// EMSCRIPTEN APIS
mod env;
mod errno;
mod exception;
mod io;
mod jmp;
mod lock;
mod math;
mod memory;
mod nullfunc;
mod process;
mod signal;
mod storage;
mod syscalls;
mod time;
mod utils;
mod varargs;

pub use self::storage::align_memory;
pub use self::utils::{allocate_cstr_on_stack, allocate_on_stack, is_emscripten_module};

// TODO: Magic number - how is this calculated?
const TOTAL_STACK: u32 = 5_242_880;
// TODO: Magic number - how is this calculated?
const DYNAMICTOP_PTR_DIFF: u32 = 1088;
// TODO: make this variable
const STATIC_BUMP: u32 = 215_536;

fn stacktop(static_bump: u32) -> u32 {
    align_memory(dynamictop_ptr(static_bump) + 4)
}

fn stack_max(static_bump: u32) -> u32 {
    stacktop(static_bump) + TOTAL_STACK
}

fn dynamic_base(static_bump: u32) -> u32 {
    align_memory(stack_max(static_bump))
}

fn dynamictop_ptr(static_bump: u32) -> u32 {
    static_bump + DYNAMICTOP_PTR_DIFF
}

pub enum InstanceEnvironment {
    EmptyInstanceEnvironment,
    EmscriptenInstanceEnvironment(EmscriptenData),
}

const EMSCRIPTEN_INSTANCE_INDEX: usize = 0;

pub struct EmscriptenModuleEnvironment {}
impl EmscriptenModuleEnvironment {
    pub fn new() -> EmscriptenModuleEnvironment {
        EmscriptenModuleEnvironment {}
    }
}

impl ModuleEnvironment for EmscriptenModuleEnvironment {
    fn after_instantiate(&self, instance: &mut Instance) {
        let data = EmscriptenData::new(&Arc::clone(&instance.module), instance);
        let instance_environment = InstanceEnvironment::EmscriptenInstanceEnvironment(data);
        instance.environments[EMSCRIPTEN_INSTANCE_INDEX] = instance_environment;
    }

    fn append_imports(&self, mut import_object: &mut Imports) {
        generate_emscripten_imports(&mut import_object);
    }
}

pub struct EmscriptenData {
    pub malloc: extern "C" fn(i32, &crate::runtime::Instance) -> u32,
    pub free: extern "C" fn(i32, &mut crate::runtime::Instance),
    pub memalign: extern "C" fn(u32, u32, &mut crate::runtime::Instance) -> u32,
    pub memset: extern "C" fn(u32, i32, u32, &mut crate::runtime::Instance) -> u32,
    pub stack_alloc: extern "C" fn(u32, &crate::runtime::Instance) -> u32,
    pub jumps: Vec<UnsafeCell<[c_int; 27]>>,
}

impl EmscriptenData {
    pub fn new(module: &Module, instance: &Instance) -> Self {
        unsafe {
            debug!("emscripten::new");

            // TODO
            //            let malloc_export = module.info.exports.get("_malloc");
            //            let free_export = module.info.exports.get("_free");
            //            let memalign_export = module.info.exports.get("_memalign");
            //            let memset_export = module.info.exports.get("_memset");
            //            let stack_alloc_export = module.info.exports.get("stackAlloc");

            let mut malloc_addr = 0 as *const u8;
            let mut free_addr = 0 as *const u8;
            let mut memalign_addr = 0 as *const u8;
            let mut memset_addr = 0 as *const u8;
            let mut stack_alloc_addr = 0 as *const u8; // as _

            // TODO
            //            if let Some(Export::Function(malloc_index)) = malloc_export {
            //                malloc_addr = instance.get_function_pointer(*malloc_index);
            //            }
            //
            //            if let Some(Export::Function(free_index)) = free_export {
            //                free_addr = instance.get_function_pointer(*free_index);
            //            }
            //
            //            if let Some(Export::Function(memalign_index)) = memalign_export {
            //                memalign_addr = instance.get_function_pointer(*memalign_index);
            //            }
            //
            //            if let Some(Export::Function(memset_index)) = memset_export {
            //                memset_addr = instance.get_function_pointer(*memset_index);
            //            }
            //
            //            if let Some(Export::Function(stack_alloc_index)) = stack_alloc_export {
            //                stack_alloc_addr = instance.get_function_pointer(*stack_alloc_index);
            //            }

            EmscriptenData {
                malloc: mem::transmute(malloc_addr),
                free: mem::transmute(free_addr),
                memalign: mem::transmute(memalign_addr),
                memset: mem::transmute(memset_addr),
                stack_alloc: mem::transmute(stack_alloc_addr),
                jumps: Vec::new(),
            }
        }
    }

    // Emscripten __ATINIT__
    pub fn atinit(&self, module: &Module, instance: &Instance) -> Result<(), String> {
        debug!("emscripten::atinit");

        // TODO
        //        if let Some(&Export::Function(environ_constructor_index)) =
        //        module.info.exports.get("___emscripten_environ_constructor")
        //        {
        //            debug!("emscripten::___emscripten_environ_constructor");
        //            let ___emscripten_environ_constructor: extern "C" fn(&Instance) =
        //                get_instance_function!(instance, environ_constructor_index);
        //            call_protected!(___emscripten_environ_constructor(&instance))
        //                .map_err(|err| format!("{}", err))?;
        //        };
        // TODO: We also need to handle TTY.init() and SOCKFS.root = FS.mount(SOCKFS, {}, null)
        Ok(())
    }

    // Emscripten __ATEXIT__
    pub fn atexit(&self, _module: &Module, _instance: &Instance) -> Result<(), String> {
        debug!("emscripten::atexit");
        use libc::fflush;
        use std::ptr;
        // Flush all open streams
        unsafe {
            fflush(ptr::null_mut());
        };
        Ok(())
    }
}

pub fn emscripten_set_up_memory(memory: &mut LinearMemory) {
    let dynamictop_ptr = dynamictop_ptr(STATIC_BUMP) as usize;
    let dynamictop_ptr_offset = dynamictop_ptr + mem::size_of::<u32>();

    // println!("value = {:?}");

    // We avoid failures of setting the u32 in our memory if it's out of bounds
    if dynamictop_ptr_offset > memory.len() {
        return; // TODO: We should panic instead?
    }

    // debug!("###### dynamic_base = {:?}", dynamic_base(STATIC_BUMP));
    // debug!("###### dynamictop_ptr = {:?}", dynamictop_ptr);
    // debug!("###### dynamictop_ptr_offset = {:?}", dynamictop_ptr_offset);

    let mem = &mut memory[dynamictop_ptr..dynamictop_ptr_offset];
    LittleEndian::write_u32(mem, dynamic_base(STATIC_BUMP));
}

macro_rules! mock_external {
    ($import:ident, $name:ident) => {{
        use crate::runtime::types::{FuncSig, Type};
        use crate::runtime::Import;
        extern "C" fn _mocked_fn() -> i32 {
            debug!("emscripten::{} <mock>", stringify!($name));
            -1
        }
        $import.add(
            "env".to_string(),
            stringify!($name).to_string(),
            Import::Func(
                _mocked_fn as _,
                FuncSig {
                    params: vec![],
                    returns: vec![Type::I32],
                },
            ),
        );
    }};
}

pub fn generate_emscripten_env() -> Imports {
    let mut import_object = Imports::new();
    generate_emscripten_imports(&mut import_object);
    import_object
}

pub fn generate_emscripten_imports(import_object: &mut Imports) {
    //    import_object.add(
    //        "spectest".to_string(),
    //        "print_i32".to_string(),
    //        Import::Func(
    //            print_i32 as _,
    //            FuncSig {
    //                params: vec![Type::I32],
    //                returns: vec![],
    //            },
    //        ),
    //    );
    //
    //    import_object.add(
    //        "spectest".to_string(),
    //        "global_i32".to_string(),
    //        Import::Global(Value::I64(GLOBAL_I32 as _)),
    //    );

    // Globals
    import_object.add(
        "env".to_string(),
        "STACKTOP".to_string(),
        Import::Global(Value::I64(stacktop(STATIC_BUMP) as _)),
    );
    import_object.add(
        "env".to_string(),
        "STACK_MAX".to_string(),
        Import::Global(Value::I64(stack_max(STATIC_BUMP) as _)),
    );
    import_object.add(
        "env".to_string(),
        "DYNAMICTOP_PTR".to_string(),
        Import::Global(Value::I64(dynamictop_ptr(STATIC_BUMP) as _)),
    );
    import_object.add(
        "global".to_string(),
        "Infinity".to_string(),
        Import::Global(Value::I64(std::f64::INFINITY.to_bits() as _)),
    );
    import_object.add(
        "global".to_string(),
        "NaN".to_string(),
        Import::Global(Value::I64(std::f64::NAN.to_bits() as _)),
    );
    import_object.add(
        "env".to_string(),
        "tableBase".to_string(),
        Import::Global(Value::I64(0)),
    );
    //    // Print functions

    import_object.add(
        "env".to_string(),
        "printf".to_string(),
        Import::Func(
            io::printf as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "putchar".to_string(),
        Import::Func(
            io::putchar as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    //    // Lock
    import_object.add(
        "env".to_string(),
        "___lock".to_string(),
        Import::Func(
            lock::___lock as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___unlock".to_string(),
        Import::Func(
            lock::___unlock as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___wait".to_string(),
        Import::Func(
            lock::___wait as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    //    // Env
    import_object.add(
        "env".to_string(),
        "_getenv".to_string(),
        Import::Func(
            env::_getenv as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_setenv".to_string(),
        Import::Func(
            env::_setenv as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_putenv".to_string(),
        Import::Func(
            env::_putenv as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_unsetenv".to_string(),
        Import::Func(
            env::_unsetenv as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_getpwnam".to_string(),
        Import::Func(
            env::_getpwnam as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_getgrnam".to_string(),
        Import::Func(
            env::_getgrnam as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___buildEnvironment".to_string(),
        Import::Func(
            env::___build_environment as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    //    // Errno
    import_object.add(
        "env".to_string(),
        "___setErrNo".to_string(),
        Import::Func(
            errno::___seterrno as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Syscalls
    import_object.add(
        "env".to_string(),
        "___syscall1".to_string(),
        Import::Func(
            syscalls::___syscall1 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall3".to_string(),
        Import::Func(
            syscalls::___syscall3 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall4".to_string(),
        Import::Func(
            syscalls::___syscall4 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall5".to_string(),
        Import::Func(
            syscalls::___syscall5 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall6".to_string(),
        Import::Func(
            syscalls::___syscall6 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall12".to_string(),
        Import::Func(
            syscalls::___syscall12 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall20".to_string(),
        Import::Func(
            syscalls::___syscall20 as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall39".to_string(),
        Import::Func(
            syscalls::___syscall39 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall40".to_string(),
        Import::Func(
            syscalls::___syscall40 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall54".to_string(),
        Import::Func(
            syscalls::___syscall54 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall57".to_string(),
        Import::Func(
            syscalls::___syscall57 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall63".to_string(),
        Import::Func(
            syscalls::___syscall63 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall64".to_string(),
        Import::Func(
            syscalls::___syscall64 as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall102".to_string(),
        Import::Func(
            syscalls::___syscall102 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall114".to_string(),
        Import::Func(
            syscalls::___syscall114 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall122".to_string(),
        Import::Func(
            syscalls::___syscall122 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall140".to_string(),
        Import::Func(
            syscalls::___syscall140 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall142".to_string(),
        Import::Func(
            syscalls::___syscall142 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall145".to_string(),
        Import::Func(
            syscalls::___syscall145 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall146".to_string(),
        Import::Func(
            syscalls::___syscall146 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall180".to_string(),
        Import::Func(
            syscalls::___syscall180 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall181".to_string(),
        Import::Func(
            syscalls::___syscall181 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall192".to_string(),
        Import::Func(
            syscalls::___syscall192 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall195".to_string(),
        Import::Func(
            syscalls::___syscall195 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall197".to_string(),
        Import::Func(
            syscalls::___syscall197 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall201".to_string(),
        Import::Func(
            syscalls::___syscall201 as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall202".to_string(),
        Import::Func(
            syscalls::___syscall202 as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall212".to_string(),
        Import::Func(
            syscalls::___syscall212 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall221".to_string(),
        Import::Func(
            syscalls::___syscall221 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall330".to_string(),
        Import::Func(
            syscalls::___syscall330 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___syscall340".to_string(),
        Import::Func(
            syscalls::___syscall340 as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Process
    import_object.add(
        "env".to_string(),
        "abort".to_string(),
        Import::Func(
            process::em_abort as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_abort".to_string(),
        Import::Func(
            process::_abort as _,
            FuncSig {
                params: vec![],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "abortStackOverflow".to_string(),
        Import::Func(
            process::abort_stack_overflow as _,
            FuncSig {
                params: vec![],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_llvm_trap".to_string(),
        Import::Func(
            process::_llvm_trap as _,
            FuncSig {
                params: vec![],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_fork".to_string(),
        Import::Func(
            process::_fork as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_exit".to_string(),
        Import::Func(
            process::_exit as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_system".to_string(),
        Import::Func(
            process::_system as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_popen".to_string(),
        Import::Func(
            process::_popen as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Signal
    import_object.add(
        "env".to_string(),
        "_sigemptyset".to_string(),
        Import::Func(
            signal::_sigemptyset as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_sigaddset".to_string(),
        Import::Func(
            signal::_sigaddset as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_sigprocmask".to_string(),
        Import::Func(
            signal::_sigprocmask as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_sigaction".to_string(),
        Import::Func(
            signal::_sigaction as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_signal".to_string(),
        Import::Func(
            signal::_signal as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Memory
    import_object.add(
        "env".to_string(),
        "abortOnCannotGrowMemory".to_string(),
        Import::Func(
            memory::abort_on_cannot_grow_memory as _,
            FuncSig {
                params: vec![],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_emscripten_memcpy_big".to_string(),
        Import::Func(
            memory::_emscripten_memcpy_big as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "enlargeMemory".to_string(),
        Import::Func(
            memory::enlarge_memory as _,
            FuncSig {
                params: vec![],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "getTotalMemory".to_string(),
        Import::Func(
            memory::get_total_memory as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___map_file".to_string(),
        Import::Func(
            memory::___map_file as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Exception
    import_object.add(
        "env".to_string(),
        "___cxa_allocate_exception".to_string(),
        Import::Func(
            exception::___cxa_allocate_exception as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___cxa_allocate_exception".to_string(),
        Import::Func(
            exception::___cxa_throw as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___cxa_throw".to_string(),
        Import::Func(
            exception::___cxa_throw as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );
    //    // NullFuncs
    import_object.add(
        "env".to_string(),
        "nullFunc_ii".to_string(),
        Import::Func(
            nullfunc::nullfunc_ii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_iii".to_string(),
        Import::Func(
            nullfunc::nullfunc_iii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_iiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_iiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_iiiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_iiiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_iiiiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_iiiiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_v".to_string(),
        Import::Func(
            nullfunc::nullfunc_v as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_vi".to_string(),
        Import::Func(
            nullfunc::nullfunc_vi as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_vii".to_string(),
        Import::Func(
            nullfunc::nullfunc_vii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_viii".to_string(),
        Import::Func(
            nullfunc::nullfunc_viii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_viiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_viiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_viiiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_viiiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "nullFunc_viiiiii".to_string(),
        Import::Func(
            nullfunc::nullfunc_viiiiii as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![],
            },
        ),
    );
    //    // Time
    import_object.add(
        "env".to_string(),
        "_gettimeofday".to_string(),
        Import::Func(
            time::_gettimeofday as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_clock_gettime".to_string(),
        Import::Func(
            time::_clock_gettime as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "___clock_gettime".to_string(),
        Import::Func(
            time::___clock_gettime as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_clock".to_string(),
        Import::Func(
            time::_clock as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_difftime".to_string(),
        Import::Func(
            time::_difftime as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_asctime".to_string(),
        Import::Func(
            time::_asctime as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_asctime_r".to_string(),
        Import::Func(
            time::_asctime_r as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_localtime".to_string(),
        Import::Func(
            time::_localtime as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_time".to_string(),
        Import::Func(
            time::_time as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_strftime".to_string(),
        Import::Func(
            time::_strftime as _,
            FuncSig {
                params: vec![Type::I32, Type::I32, Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_localtime_r".to_string(),
        Import::Func(
            time::_localtime_r as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_getpagesize".to_string(),
        Import::Func(
            env::_getpagesize as _,
            FuncSig {
                params: vec![],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_sysconf".to_string(),
        Import::Func(
            env::_sysconf as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    //    // Math
    import_object.add(
        "env".to_string(),
        "_llvm_log10_f64".to_string(),
        Import::Func(
            math::_llvm_log10_f64 as _,
            FuncSig {
                params: vec![Type::F64],
                returns: vec![Type::F64],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "_llvm_log2_f64".to_string(),
        Import::Func(
            math::_llvm_log2_f64 as _,
            FuncSig {
                params: vec![Type::F64],
                returns: vec![Type::F64],
            },
        ),
    );
    import_object.add(
        "asm2wasm".to_string(),
        "f64-rem".to_string(),
        Import::Func(
            math::f64_rem as _,
            FuncSig {
                params: vec![Type::F64, Type::F64],
                returns: vec![Type::F64],
            },
        ),
    );
    //
    import_object.add(
        "env".to_string(),
        "__setjmp".to_string(),
        Import::Func(
            jmp::__setjmp as _,
            FuncSig {
                params: vec![Type::I32],
                returns: vec![Type::I32],
            },
        ),
    );
    import_object.add(
        "env".to_string(),
        "__longjmp".to_string(),
        Import::Func(
            jmp::__longjmp as _,
            FuncSig {
                params: vec![Type::I32, Type::I32],
                returns: vec![],
            },
        ),
    );

    mock_external!(import_object, _waitpid);
    mock_external!(import_object, _utimes);
    mock_external!(import_object, _usleep);
    // mock_external!(import_object, _time);
    // mock_external!(import_object, _sysconf);
    // mock_external!(import_object, _strftime);
    mock_external!(import_object, _sigsuspend);
    // mock_external!(import_object, _sigprocmask);
    // mock_external!(import_object, _sigemptyset);
    // mock_external!(import_object, _sigaddset);
    // mock_external!(import_object, _sigaction);
    mock_external!(import_object, _setitimer);
    mock_external!(import_object, _setgroups);
    mock_external!(import_object, _setgrent);
    mock_external!(import_object, _sem_wait);
    mock_external!(import_object, _sem_post);
    mock_external!(import_object, _sem_init);
    mock_external!(import_object, _sched_yield);
    mock_external!(import_object, _raise);
    mock_external!(import_object, _mktime);
    // mock_external!(import_object, _localtime_r);
    // mock_external!(import_object, _localtime);
    mock_external!(import_object, _llvm_stacksave);
    mock_external!(import_object, _llvm_stackrestore);
    mock_external!(import_object, _kill);
    mock_external!(import_object, _gmtime_r);
    // mock_external!(import_object, _gettimeofday);
    // mock_external!(import_object, _getpagesize);
    mock_external!(import_object, _getgrent);
    mock_external!(import_object, _getaddrinfo);
    // mock_external!(import_object, _fork);
    // mock_external!(import_object, _exit);
    mock_external!(import_object, _execve);
    mock_external!(import_object, _endgrent);
    // mock_external!(import_object, _clock_gettime);
    mock_external!(import_object, ___syscall97);
    mock_external!(import_object, ___syscall91);
    mock_external!(import_object, ___syscall85);
    mock_external!(import_object, ___syscall75);
    mock_external!(import_object, ___syscall66);
    // mock_external!(import_object, ___syscall64);
    // mock_external!(import_object, ___syscall63);
    // mock_external!(import_object, ___syscall60);
    // mock_external!(import_object, ___syscall54);
    // mock_external!(import_object, ___syscall39);
    mock_external!(import_object, ___syscall38);
    // mock_external!(import_object, ___syscall340);
    mock_external!(import_object, ___syscall334);
    mock_external!(import_object, ___syscall300);
    mock_external!(import_object, ___syscall295);
    mock_external!(import_object, ___syscall272);
    mock_external!(import_object, ___syscall268);
    // mock_external!(import_object, ___syscall221);
    mock_external!(import_object, ___syscall220);
    // mock_external!(import_object, ___syscall212);
    // mock_external!(import_object, ___syscall201);
    mock_external!(import_object, ___syscall199);
    // mock_external!(import_object, ___syscall197);
    mock_external!(import_object, ___syscall196);
    // mock_external!(import_object, ___syscall195);
    mock_external!(import_object, ___syscall194);
    mock_external!(import_object, ___syscall191);
    // mock_external!(import_object, ___syscall181);
    // mock_external!(import_object, ___syscall180);
    mock_external!(import_object, ___syscall168);
    // mock_external!(import_object, ___syscall146);
    // mock_external!(import_object, ___syscall145);
    // mock_external!(import_object, ___syscall142);
    mock_external!(import_object, ___syscall140);
    // mock_external!(import_object, ___syscall122);
    // mock_external!(import_object, ___syscall102);
    // mock_external!(import_object, ___syscall20);
    mock_external!(import_object, ___syscall15);
    mock_external!(import_object, ___syscall10);
    mock_external!(import_object, _dlopen);
    mock_external!(import_object, _dlclose);
    mock_external!(import_object, _dlsym);
    mock_external!(import_object, _dlerror);
}
