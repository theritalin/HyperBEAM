#include <erl_nif.h>
#include <wasm_c_api.h>
#include <wasm_export.h>
#include <string.h>
#include <setjmp.h>
#include <stdarg.h>
#include <stdint.h>
#include "cu_erwamr_imports.h"

typedef struct {
    char* module_name;
    char* field_name;
    char* signature;
    void* attachment;
    uint32_t (*func)(wasm_exec_env_t exec_env, ...);
} ImportHook;

typedef struct {
    char* module_name;
    ImportHook* import_hooks;
    int count;
} HookLib;

typedef struct {
    const char* module_name;
    const char* field_name;
    ERL_NIF_TERM args;
    ERL_NIF_TERM signature;
    char ret_type;
    wasm_val_t result;
    int has_result;
} ImportCallInfo;

typedef struct {
    wasm_instance_t* instance;
    wasm_module_t* module;
    wasm_memory_t* memory;
    wasm_store_t* store;
    jmp_buf env_buffer;
    int import_hit;
    ImportCallInfo current_import;
    int is_running;
} WasmInstanceResource;

typedef struct {
    wasm_module_t* module;
    wasm_store_t* store;
    HookLib* hook_libs;
    int lib_count;
} WasmModuleResource;

static ErlNifResourceType* WASM_MODULE_RESOURCE;
static ErlNifResourceType* WASM_INSTANCE_RESOURCE;

static ERL_NIF_TERM atom_ok;
static ERL_NIF_TERM atom_error;

#define NIF_DEBUG(format, ...) debug_print(__FILE__, __LINE__, format, ##__VA_ARGS__)

void debug_print(const char* file, int line, const char* format, ...) {
    va_list args;
    va_start(args, format);
    printf("[%s:%d] NIF_DEBUG: ", file, line);
    vprintf(format, args);
    printf("\n");
    va_end(args);
}

int wasm_val_to_erlang(ErlNifEnv* env, wasm_val_t* val, ERL_NIF_TERM* term) {
    switch (val->kind) {
        case WASM_I32: *term = enif_make_int(env, val->of.i32); break;
        case WASM_I64: *term = enif_make_int64(env, val->of.i64); break;
        case WASM_F32: *term = enif_make_double(env, (double)val->of.f32); break;
        case WASM_F64: *term = enif_make_double(env, val->of.f64); break;
        default: return 0;
    }
    return 1;
}

const char* wasm_externtype_kind_to_string(wasm_externkind_t kind) {
    switch (kind) {
        case WASM_EXTERN_FUNC: return "func";
        case WASM_EXTERN_GLOBAL: return "global";
        case WASM_EXTERN_TABLE: return "table";
        case WASM_EXTERN_MEMORY: return "memory";
        default: return "unknown";
    }
}

int erlang_to_wasm_val(ErlNifEnv* env, ERL_NIF_TERM term, wasm_val_t* val, wasm_valkind_t expected_kind) {
    switch (expected_kind) {
        case WASM_I32:
            {
                int temp;
                if (enif_get_int(env, term, &temp)) {
                    val->kind = WASM_I32;
                    val->of.i32 = temp;
                    return 1;
                }
            }
            break;
        case WASM_I64:
            {
                long temp;
                if (enif_get_int64(env, term, &temp)) {
                    val->kind = WASM_I64;
                    val->of.i64 = temp;
                    return 1;
                }
            }
            break;
        case WASM_F32:
            {
                double temp;
                if (enif_get_double(env, term, &temp)) {
                    val->kind = WASM_F32;
                    val->of.f32 = (float)temp;
                    return 1;
                }
            }
            break;
        case WASM_F64:
            if (enif_get_double(env, term, &val->of.f64)) {
                val->kind = WASM_F64;
                return 1;
            }
            break;
        default:
            return 0;
    }
    return 0;
}

wasm_valkind_t erlang_to_wasm_val_char(ErlNifEnv* env, ERL_NIF_TERM term, wasm_val_t* val, char kind) {
    switch (kind) {
        case 'i': return erlang_to_wasm_val(env, term, val, WASM_I32);
        case 'I': return erlang_to_wasm_val(env, term, val, WASM_I64);
        case 'f': return erlang_to_wasm_val(env, term, val, WASM_F32);
        case 'F': return erlang_to_wasm_val(env, term, val, WASM_F64);
        case 'R': return erlang_to_wasm_val(env, term, val, WASM_EXTERNREF);
        case 'V': return erlang_to_wasm_val(env, term, val, WASM_V128);
        case 'c': return erlang_to_wasm_val(env, term, val, WASM_FUNCREF);
        default: return WASM_I32;
    }
}

uint32_t generic_import_handler(wasm_exec_env_t exec_env, ...) {
    NIF_DEBUG("Entering generic_import_handler");
    ImportHook* import_hook = (ImportHook*)wasm_runtime_get_function_attachment(exec_env);
    if (!import_hook) {
        NIF_DEBUG("import_hook is NULL");
        return 0;
    }
    return 0;
    const char* module_name = import_hook->module_name;
    const char* field_name = import_hook->field_name;
    const char* signature = import_hook->signature;
    NIF_DEBUG("Successfully got import_hook %p", import_hook);
    NIF_DEBUG("module_name: %p %c", module_name, *module_name);
    NIF_DEBUG("field_name: %p %c", field_name, *field_name);
    NIF_DEBUG("signature: %p %c", signature, *signature);
    wasm_module_inst_t module_inst = wasm_runtime_get_module_inst(exec_env);
    WasmInstanceResource* instance_res = wasm_runtime_get_custom_data(module_inst);

    va_list args;
    va_start(args, exec_env);

    ErlNifEnv* env = enif_alloc_env();

    // Parse the signature and convert arguments to Erlang terms
    ERL_NIF_TERM arg_list = enif_make_list(env, 0);
    const char* sig_ptr = signature + 1;  // Skip the opening parenthesis
    while (*sig_ptr != ')') {
        ERL_NIF_TERM term;
        switch (*sig_ptr) {
            case 'i':
                term = enif_make_int(env, va_arg(args, int32_t));
                break;
            case 'I':
                term = enif_make_int64(env, va_arg(args, int64_t));
                break;
            case 'f':
                term = enif_make_double(env, va_arg(args, double));
                break;
            case 'F':
                term = enif_make_double(env, va_arg(args, double));
                break;
            default:
                NIF_DEBUG("Unknown type in signature: %c", *sig_ptr);
                va_end(args);
                enif_free_env(env);
                return 0;
        }
        arg_list = enif_make_list_cell(env, term, arg_list);
        sig_ptr++;
    }

    sig_ptr += 1;

    char ret_type = *sig_ptr ? *sig_ptr : 0;

    ERL_NIF_TERM reversed_list;
    if (!enif_make_reverse_list(env, arg_list, &reversed_list)) {
        NIF_DEBUG("Failed to reverse list");
        va_end(args);
        enif_free_env(env);
        return 0;
    }
    arg_list = reversed_list;

    va_end(args);

    // Set current import information
    instance_res->current_import.module_name = module_name;
    instance_res->current_import.field_name = field_name;
    instance_res->current_import.args = arg_list;
    instance_res->current_import.signature = enif_make_string(env, signature, ERL_NIF_LATIN1);
    instance_res->current_import.ret_type = ret_type;
    instance_res->current_import.has_result = 0;
    
    if (setjmp(instance_res->env_buffer) == 0) {
        // First time through, jump back to call_nif
        instance_res->import_hit = 1;
        longjmp(instance_res->env_buffer, 1);
    }
    
    // When we return here after resuming, convert the result back to C type
    if (instance_res->current_import.has_result) {
        // Convert the Erlang term result back to the appropriate C type
        switch (signature[strlen(signature) - 1]) {
            case 'i':
                return instance_res->current_import.result.of.i32;
            case 'I':
                return instance_res->current_import.result.of.i64;
            // Add more cases as needed for other return types
            default:
                // Handle unknown type or error
                return 0;
        }
    }

    enif_free_env(env);
    return 0;
}

#define CLEANUP_AND_RETURN_ERROR(env, message) do { \
    cleanup_resources(&args, &results, &exports, &export_types); \
    return enif_make_tuple2(env, atom_error, enif_make_string(env, message, ERL_NIF_LATIN1)); \
} while(0)

static void cleanup_resources(wasm_val_vec_t* args, wasm_val_vec_t* results, 
                              wasm_extern_vec_t* exports, wasm_exporttype_vec_t* export_types) {
    if (args) wasm_val_vec_delete(args);
    if (results) wasm_val_vec_delete(results);
    if (exports) wasm_extern_vec_delete(exports);
    if (export_types) wasm_exporttype_vec_delete(export_types);
}

static void cleanup_wasm_instance(ErlNifEnv* env, void* obj) {
    WasmInstanceResource* res = (WasmInstanceResource*)obj;
    if (res->instance) wasm_instance_delete(res->instance);
    if (res->store) wasm_store_delete(res->store);
}

// Helper function to create a binary term without null terminator
ERL_NIF_TERM make_binary_term(ErlNifEnv* env, const char* data, size_t size) {
    // Check if the last character is a null terminator
    if (size > 0 && data[size - 1] == '\0') {
        size--; // Decrease size to exclude null terminator
    }
    return enif_make_binary(env, &(ErlNifBinary){.size = size, .data = (unsigned char*)data});
}

// Helper function to convert wasm_valtype_t to char
char wasm_valtype_kind_to_char(const wasm_valtype_t* valtype) {
    switch (wasm_valtype_kind(valtype)) {
        case WASM_I32: return 'i';
        case WASM_I64: return 'I';
        case WASM_F32: return 'f';
        case WASM_F64: return 'F';
        case WASM_EXTERNREF: return 'R';
        case WASM_V128: return 'V';
        case WASM_FUNCREF: return 'c';
        default: return '?';
    }
}

int get_function_sig(const wasm_externtype_t* type, char* type_str) {
    if (wasm_externtype_kind(type) == WASM_EXTERN_FUNC) {
        const wasm_functype_t* functype = wasm_externtype_as_functype_const(type);
        const wasm_valtype_vec_t* params = wasm_functype_params(functype);
        const wasm_valtype_vec_t* results = wasm_functype_results(functype);

        type_str[0] = '(';
        size_t offset = 1;

        for (size_t i = 0; i < params->size; ++i) {
            type_str[offset++] = wasm_valtype_kind_to_char(params->data[i]);
        }
        type_str[offset++] = ')';

        for (size_t i = 0; i < results->size; ++i) {
            type_str[offset++] = wasm_valtype_kind_to_char(results->data[i]);
        }
        type_str[offset] = '\0';

        return 1;
    }
    return 0;
}

// New helper function to get function type in the "(iIiI)i" format
ERL_NIF_TERM get_function_type_term(ErlNifEnv* env, const wasm_externtype_t* type) {
    char type_str[256];
    if(get_function_sig(type, type_str)) {
        return make_binary_term(env, type_str, strlen(type_str));
    }
    return enif_make_atom(env, "undefined");
}

// Function to find or create a HookLib for a given module name
static HookLib* find_or_create_hook_lib(HookLib** hook_libs, int* hook_libs_count, const char* module_name) {
    for (int i = 0; i < *hook_libs_count; i++) {
        if (strcmp((*hook_libs)[i].module_name, module_name) == 0) {
            return &(*hook_libs)[i];
        }
    }

    // If no existing HookLib, create a new one
    (*hook_libs_count)++;
    *hook_libs = realloc(*hook_libs, (*hook_libs_count) * sizeof(HookLib));
    HookLib* new_lib = &(*hook_libs)[*hook_libs_count - 1];

    new_lib->module_name = strdup(module_name);
    new_lib->import_hooks = NULL;
    new_lib->count = 0;

    return new_lib;
}

// Function to split the big HookLib into multiple HookLibs by module_name
HookLib* split_hooklib_by_module(HookLib* big_hook_lib, int* out_hook_libs_count) {
    HookLib* split_hook_libs = NULL;
    *out_hook_libs_count = 0;

    for (int i = 0; i < big_hook_lib->count; i++) {
        ImportHook* hook = &big_hook_lib->import_hooks[i];

        // Find or create a HookLib for the current module_name
        HookLib* lib = find_or_create_hook_lib(&split_hook_libs, out_hook_libs_count, hook->module_name);

        // Add the current ImportHook to the HookLib
        lib->count++;
        lib->import_hooks = realloc(lib->import_hooks, lib->count * sizeof(ImportHook));
        lib->import_hooks[lib->count - 1] = *hook;
    }

    return split_hook_libs;
}

static ERL_NIF_TERM load_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    ErlNifBinary wasm_binary;
    if (argc != 1 || !enif_inspect_binary(env, argv[0], &wasm_binary)) {
        return enif_make_badarg(env);
    }

    // Run full init
    RuntimeInitArgs init_args = {};
    init_args.running_mode = Mode_Fast_JIT;
    wasm_runtime_full_init(&init_args);

    wasm_engine_t* engine = wasm_engine_new();
    wasm_store_t* store = wasm_store_new(engine);

    wasm_byte_vec_t binary;
    wasm_byte_vec_new(&binary, wasm_binary.size, (const wasm_byte_t*)wasm_binary.data);

    wasm_module_t* module = wasm_module_new(store, &binary);
    if (!module) {
        wasm_byte_vec_delete(&binary);
        wasm_store_delete(store);
        wasm_engine_delete(engine);
        return enif_make_tuple2(env, atom_error, enif_make_string(env, "Failed to compile module", ERL_NIF_LATIN1));
    }

    wasm_byte_vec_delete(&binary);

    // Get imports
    wasm_importtype_vec_t imports;
    wasm_module_imports(module, &imports);

    // Get exports
    wasm_exporttype_vec_t exports;
    wasm_module_exports(module, &exports);

    // Create Erlang lists for imports and exports
    ERL_NIF_TERM import_list = enif_make_list(env, 0);
    ERL_NIF_TERM export_list = enif_make_list(env, 0);

    // TODO: Free this memory...
    HookLib* hook_lib = malloc(sizeof(HookLib));
    hook_lib->count = imports.size;
    hook_lib->import_hooks = malloc(imports.size * sizeof(ImportHook));

    // Process imports
    for (size_t i = 0; i < imports.size; ++i) {
        const wasm_importtype_t* import = imports.data[i];
        const wasm_name_t* module_name = wasm_importtype_module(import);
        const wasm_name_t* name = wasm_importtype_name(import);
        const wasm_externtype_t* type = wasm_importtype_type(import);

        ERL_NIF_TERM module_name_term = make_binary_term(env, module_name->data, module_name->size);
        ERL_NIF_TERM name_term = make_binary_term(env, name->data, name->size);
        ERL_NIF_TERM type_term = enif_make_atom(env, wasm_externtype_kind_to_string(wasm_externtype_kind(type)));
        ERL_NIF_TERM func_type_term = get_function_type_term(env, type);

        ERL_NIF_TERM import_tuple = enif_make_tuple4(env, type_term, module_name_term, name_term, func_type_term);
        import_list = enif_make_list_cell(env, import_tuple, import_list);

        char* type_str = malloc(64);
        get_function_sig(type, type_str);

        hook_lib->import_hooks[i].module_name = strdup(module_name->data);
        hook_lib->import_hooks[i].field_name = strdup(name->data);
        hook_lib->import_hooks[i].func = generic_import_handler;
        hook_lib->import_hooks[i].signature = strdup(type_str);

        ImportHook* attachment = malloc(sizeof(ImportHook));
        attachment->module_name = module_name->data;
        attachment->field_name = name->data;
        attachment->signature = strdup(type_str);
        hook_lib->import_hooks[i].attachment = (void*)attachment;

        NIF_DEBUG("Added ImportHook: %s.%s (%s)", hook_lib->import_hooks[i].module_name, hook_lib->import_hooks[i].field_name, type_str);
    }

    int lib_count;
    HookLib* hook_libs = split_hooklib_by_module(hook_lib, &lib_count);

    NIF_DEBUG("Split hook libs into modules: %d", lib_count);

    // Process exports
    for (size_t i = 0; i < exports.size; ++i) {
        const wasm_exporttype_t* export = exports.data[i];
        const wasm_name_t* name = wasm_exporttype_name(export);
        const wasm_externtype_t* type = wasm_exporttype_type(export);

        ERL_NIF_TERM name_term = make_binary_term(env, name->data, name->size);
        ERL_NIF_TERM type_term = enif_make_atom(env, wasm_externtype_kind_to_string(wasm_externtype_kind(type)));
        ERL_NIF_TERM func_type_term = get_function_type_term(env, type);

        ERL_NIF_TERM export_tuple = enif_make_tuple3(env, type_term, name_term, func_type_term);
        export_list = enif_make_list_cell(env, export_tuple, export_list);
    }

    // Clean up
    wasm_importtype_vec_delete(&imports);
    wasm_exporttype_vec_delete(&exports);

    WasmModuleResource* module_res = enif_alloc_resource(WASM_MODULE_RESOURCE, sizeof(WasmModuleResource));
    module_res->module = module;
    module_res->store = store;
    module_res->hook_libs = hook_libs;
    module_res->lib_count = lib_count;

    ERL_NIF_TERM module_term = enif_make_resource(env, module_res);
    enif_release_resource(module_res);

    // Return the module term along with import and export lists
    return enif_make_tuple4(env, atom_ok, module_term, import_list, export_list);
}

wasm_memory_t* find_memory_export(const wasm_instance_t* instance) {
    wasm_memory_t* memory = NULL;

    // Get the exports from the instance
    wasm_extern_vec_t instance_exports;
    wasm_instance_exports(instance, &instance_exports);

    // Iterate over the exports to find the memory
    for (size_t i = 0; i < instance_exports.size; i++) {
        wasm_extern_t* export = instance_exports.data[i];

        // Check if the export is of memory kind
        if (wasm_extern_kind(export) == WASM_EXTERN_MEMORY) {
            // Cast the export to wasm_memory_t*
            memory = wasm_extern_as_memory(export);
            break; // Stop after finding the first memory
        }
    }

    // Clean up
    wasm_extern_vec_delete(&instance_exports);

    return memory;
}

static ERL_NIF_TERM instantiate_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    if (argc != 2) return enif_make_badarg(env);

    WasmModuleResource* module_res;
    if (!enif_get_resource(env, argv[0], WASM_MODULE_RESOURCE, (void**)&module_res)) {
        return enif_make_badarg(env);
    }

    wasm_extern_vec_t imports = WASM_EMPTY_VEC;
    wasm_instance_t* instance = wasm_instance_new(module_res->store, module_res->module, &imports, NULL);

    if (!instance) {
        return enif_make_tuple2(env, atom_error, enif_make_string(env, "Failed to create WASM instance", ERL_NIF_LATIN1));
    }

    // Register hook libs
    for (int i = 0; i < module_res->lib_count; i++) {
        HookLib* lib = &module_res->hook_libs[i];
        int n_native_symbols = lib->count;
        NativeSymbol* native_symbols = malloc(n_native_symbols * sizeof(NativeSymbol));
        for (int j = 0; j < n_native_symbols; j++) {
            native_symbols[j].symbol = strdup(lib->import_hooks[j].field_name);
            native_symbols[j].func_ptr = generic_import_handler;
            native_symbols[j].signature = strdup(lib->import_hooks[j].signature);
            ImportHook* attachment = malloc(sizeof(ImportHook));
            attachment->module_name = strdup(lib->module_name);
            attachment->field_name = strdup(lib->import_hooks[j].field_name);
            attachment->signature = strdup(lib->import_hooks[j].signature);
            native_symbols[j].attachment = attachment;
            NIF_DEBUG("Registering hook: %s.%s (%s) => %p", lib->module_name, lib->import_hooks[j].field_name, native_symbols[j].signature, native_symbols[j].func_ptr);
            NIF_DEBUG("Hook string lengths: sig %d, module %d, field %d", strlen(native_symbols[j].signature), strlen(lib->module_name), strlen(lib->import_hooks[j].field_name));
        }
        NIF_DEBUG("Registered hook lib: %s (%d)", lib->module_name, n_native_symbols);
        if (!wasm_runtime_register_natives(lib->module_name,
                                           native_symbols,
                                           n_native_symbols)) {
            wasm_instance_delete(instance);
            return enif_make_tuple2(env, atom_error, enif_make_string(env, "Failed to register hook libs", ERL_NIF_LATIN1));
        }
    }

    //register_native_symbols();

    wasm_importtype_vec_t check_imports;
    wasm_module_imports(module_res->module, &check_imports);
    for (size_t i = 0; i < check_imports.size; ++i) {
        const wasm_importtype_t* import = check_imports.data[i];
        const wasm_name_t* module_name = wasm_importtype_module(import);
        const wasm_name_t* name = wasm_importtype_name(import);
        NIF_DEBUG("Import: %s.%s => %d", module_name->data, name->data, wasm_runtime_is_import_func_linked(module_name->data, name->data));
    }

    WasmInstanceResource* instance_res = enif_alloc_resource(WASM_INSTANCE_RESOURCE, sizeof(WasmInstanceResource));
    instance_res->instance = instance;
    instance_res->module = module_res->module;
    instance_res->store = module_res->store;
    instance_res->is_running = 0;  // Initialize the flag
    instance_res->memory = find_memory_export(instance);

    ERL_NIF_TERM instance_term = enif_make_resource(env, instance_res);
    enif_release_resource(instance_res);

    return enif_make_tuple2(env, atom_ok, instance_term);
}

static ERL_NIF_TERM call_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    if (argc != 3) return enif_make_badarg(env);

    WasmInstanceResource* instance_res;
    if (!enif_get_resource(env, argv[0], WASM_INSTANCE_RESOURCE, (void**)&instance_res)) {
        return enif_make_badarg(env);
    }

    NIF_DEBUG("Call time");

    wasm_importtype_vec_t check_imports;
    wasm_module_imports(instance_res->module, &check_imports);
    for (size_t i = 0; i < check_imports.size; ++i) {
        const wasm_importtype_t* import = check_imports.data[i];
        const wasm_name_t* module_name = wasm_importtype_module(import);
        const wasm_name_t* name = wasm_importtype_name(import);
        NIF_DEBUG("Call time imports: %s.%s => %d", module_name->data, name->data, wasm_runtime_is_import_func_linked(module_name->data, name->data));
    }

    // Check if the instance is already running
    if (instance_res->is_running) {
        return enif_make_tuple2(env, atom_error, enif_make_atom(env, "instance_already_running"));
    }
    // Set the running flag
    instance_res->is_running = 1;

    ErlNifBinary function_name_binary;
    if (!enif_inspect_binary(env, argv[1], &function_name_binary)) {
        instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
        return enif_make_badarg(env);
    }

    // Ensure the binary is null-terminated for C string operations
    char* function_name = enif_alloc(function_name_binary.size + 1);
    memcpy(function_name, function_name_binary.data, function_name_binary.size);
    function_name[function_name_binary.size] = '\0';

    wasm_extern_vec_t exports;
    wasm_instance_exports(instance_res->instance, &exports);

    wasm_exporttype_vec_t export_types;
    wasm_module_exports(instance_res->module, &export_types);

    wasm_func_t* func = NULL;
    const wasm_functype_t* func_type = NULL;
    for (size_t i = 0; i < exports.size; ++i) {
        wasm_extern_t* ext = exports.data[i];
        if (wasm_extern_kind(ext) == WASM_EXTERN_FUNC) {
            const wasm_name_t* name = wasm_exporttype_name(export_types.data[i]);
            if (name && name->size == strlen(function_name) + 1 && 
                strncmp(name->data, function_name, name->size - 1) == 0) {
                func = wasm_extern_as_func(ext);
                func_type = wasm_func_type(func);
                break;
            }
        }
    }

    if (!func) {
        cleanup_resources(NULL, NULL, &exports, &export_types);
        instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
        return enif_make_tuple2(env, atom_error, enif_make_string(env, "Function not found", ERL_NIF_LATIN1));
    }

    const wasm_valtype_vec_t* param_types = wasm_functype_params(func_type);
    const wasm_valtype_vec_t* result_types = wasm_functype_results(func_type);

    ERL_NIF_TERM arg_list = argv[2];
    unsigned arg_count;
    if (!enif_get_list_length(env, arg_list, &arg_count) || param_types->size != arg_count) {
        cleanup_resources(NULL, NULL, &exports, &export_types);
        instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
        return enif_make_tuple2(env, atom_error, enif_make_string(env, "Invalid argument count", ERL_NIF_LATIN1));
    }

    wasm_val_vec_t args, results;
    wasm_val_vec_new_uninitialized(&args, param_types->size);
    wasm_val_vec_new_uninitialized(&results, result_types->size);

    ERL_NIF_TERM head, tail = arg_list;
    for (size_t i = 0; i < param_types->size; ++i) {
        if (!enif_get_list_cell(env, tail, &head, &tail) ||
            !erlang_to_wasm_val(env, head, &args.data[i], wasm_valtype_kind(param_types->data[i]))) {
            cleanup_resources(&args, &results, &exports, &export_types);
            instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
            return enif_make_tuple2(env, atom_error, enif_make_string(env, "Failed to convert argument", ERL_NIF_LATIN1));
        }
    }

    if (setjmp(instance_res->env_buffer) == 0) {
        // Normal execution
        wasm_trap_t* trap = wasm_func_call(func, &args, &results);

        if (trap) {
            wasm_message_t message;
            wasm_trap_message(trap, &message);
            ERL_NIF_TERM error_term = enif_make_tuple2(env, atom_error, enif_make_string(env, message.data, ERL_NIF_LATIN1));
            wasm_trap_delete(trap);
            wasm_byte_vec_delete(&message);
            cleanup_resources(&args, &results, &exports, &export_types);
            instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
            return error_term;
        }

        ERL_NIF_TERM result_term;
        if (results.size == 1 && wasm_val_to_erlang(env, &results.data[0], &result_term)) {
            cleanup_resources(&args, &results, &exports, &export_types);
            instance_res->is_running = 0;  // Clear the flag before returning
            return enif_make_tuple2(env, atom_ok, result_term);
        } else {
            cleanup_resources(&args, &results, &exports, &export_types);
            instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
            return enif_make_tuple2(env, atom_error, enif_make_string(env, "Unexpected result", ERL_NIF_LATIN1));
        }
    } else {
        // Import hit
        // Don't clear the running flag here, as we expect a resume_nif call
        return enif_make_tuple4(env,
            enif_make_atom(env, "import"),
            enif_make_atom(env, instance_res->current_import.module_name),
            enif_make_atom(env, instance_res->current_import.field_name),
            enif_make_tuple2(env, instance_res->current_import.args, instance_res->current_import.signature)
        );
    }

    // This point is never reached
    return enif_make_atom(env, "error");
}

static ERL_NIF_TERM resume_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    if (argc != 2) return enif_make_badarg(env);

    WasmInstanceResource* instance_res;
    if (!enif_get_resource(env, argv[0], WASM_INSTANCE_RESOURCE, (void**)&instance_res)) {
        return enif_make_badarg(env);
    }

    // Check if the instance is actually running
    if (!instance_res->is_running) {
        return enif_make_tuple2(env, atom_error, enif_make_atom(env, "instance_not_running"));
    }

    // Convert Erlang term to WASM value
    if (!erlang_to_wasm_val_char(env, argv[1], &instance_res->current_import.result, instance_res->current_import.ret_type)) {
        instance_res->is_running = 0;  // Clear the flag if we're returning due to an error
        return enif_make_tuple2(env, atom_error, enif_make_atom(env, "invalid_result"));
    }

    instance_res->current_import.has_result = 1;

    // Jump back to generic_import_handler
    longjmp(instance_res->env_buffer, 1);

    // This point is never reached
    return enif_make_atom(env, "error");
}

static int nif_load(ErlNifEnv* env, void** priv_data, ERL_NIF_TERM load_info) {
    atom_ok = enif_make_atom(env, "ok");
    atom_error = enif_make_atom(env, "error");

    WASM_MODULE_RESOURCE = enif_open_resource_type(env, NULL, "wasm_module_resource", NULL, ERL_NIF_RT_CREATE | ERL_NIF_RT_TAKEOVER, NULL);
    if (!WASM_MODULE_RESOURCE) return -1;

    WASM_INSTANCE_RESOURCE = enif_open_resource_type(env, NULL, "wasm_instance_resource", cleanup_wasm_instance, ERL_NIF_RT_CREATE | ERL_NIF_RT_TAKEOVER, NULL);
    if (!WASM_INSTANCE_RESOURCE) return -1;

    return 0;
}

// NIF function to read WASM memory
static ERL_NIF_TERM read_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    WasmInstanceResource* instance_res;
    uint32_t offset;
    uint32_t length;

    if (argc != 3 || !enif_get_resource(env, argv[0], WASM_INSTANCE_RESOURCE, (void**)&instance_res)
        || !enif_get_uint(env, argv[1], &offset) || !enif_get_uint(env, argv[2], &length)) {
        return enif_make_badarg(env);
    }

    byte_t* data = wasm_memory_data(instance_res->memory);
    size_t data_size = wasm_memory_data_size(instance_res->memory);

    if (offset + length > data_size) {
        return enif_make_tuple2(env, enif_make_atom(env, "error"), enif_make_atom(env, "access_out_of_bounds"));
    }

    ERL_NIF_TERM binary_term;
    unsigned char* binary_data = enif_make_new_binary(env, length, &binary_term);
    memcpy(binary_data, data + offset, length);

    return enif_make_tuple2(env, enif_make_atom(env, "ok"), binary_term);
}

// NIF function to write WASM memory
static ERL_NIF_TERM write_nif(ErlNifEnv* env, int argc, const ERL_NIF_TERM argv[]) {
    WasmInstanceResource* instance_res;
    uint32_t offset;
    ErlNifBinary input_binary;

    if (argc != 3 || !enif_get_resource(env, argv[0], WASM_INSTANCE_RESOURCE, (void**)&instance_res)
        || !enif_get_uint(env, argv[1], &offset) || !enif_inspect_binary(env, argv[2], &input_binary)) {
        return enif_make_badarg(env);
    }

    byte_t* data = wasm_memory_data(instance_res->memory);
    size_t data_size = wasm_memory_data_size(instance_res->memory);

    if (offset + input_binary.size > data_size) {
        return enif_make_tuple2(env, enif_make_atom(env, "error"), enif_make_atom(env, "access_out_of_bounds"));
    }

    memcpy(data + offset, input_binary.data, input_binary.size);

    return enif_make_atom(env, "ok");
}

static ErlNifFunc nif_funcs[] = {
    {"load_nif", 1, load_nif},
    {"instantiate_nif", 2, instantiate_nif},
    {"call_nif", 3, call_nif},
    {"resume_nif", 2, resume_nif},
    {"read_nif", 3, read_nif},
    {"write_nif", 3, write_nif}
};

ERL_NIF_INIT(cu_erwamr, nif_funcs, nif_load, NULL, NULL, NULL)