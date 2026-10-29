#include "usagi.h"

#include <jnivm.h>

#include <exception>
#include <memory>
#include <string>

struct UsagiVm {
    std::unique_ptr<jnivm::VM> inner;
};

namespace {

thread_local std::string last_error;

void clear_error() noexcept {
    last_error.clear();
}

void set_error(const char *message) noexcept {
    try {
        last_error = message ? message : "unknown C++ exception";
    } catch (...) {
        last_error.clear();
    }
}

bool valid_vm(UsagiVm *vm) noexcept {
    if (!vm || !vm->inner) {
        set_error("invalid UsagiVm handle");
        return false;
    }

    return true;
}

} // namespace

extern "C" {

const char *usagi_last_error(void) {
    return last_error.c_str();
}

UsagiVm *usagi_vm_create(void) {
    try {
        auto handle = std::make_unique<UsagiVm>();
        handle->inner = std::make_unique<jnivm::VM>();

        clear_error();
        return handle.release();
    } catch (const std::exception &e) {
        set_error(e.what());
    } catch (...) {
        set_error("failed to create JNI VM");
    }

    return nullptr;
}

void usagi_vm_destroy(UsagiVm *vm) {
    delete vm;
}

JavaVM *usagi_vm_get_java_vm(UsagiVm *vm) {
    if (!valid_vm(vm)) {
        return nullptr;
    }

    clear_error();
    return vm->inner->GetJavaVM();
}

JNIEnv *usagi_vm_get_jni_env(UsagiVm *vm) {
    if (!valid_vm(vm)) {
        return nullptr;
    }

    JNIEnv *env = nullptr;
    JavaVM *java_vm = vm->inner->GetJavaVM();

    const jint status = java_vm->GetEnv(
        reinterpret_cast<void **>(&env),
        JNI_VERSION_1_6
    );

    if (status != JNI_OK) {
        set_error(
            "current thread is not attached, call "
            "usagi_vm_attach_current_thread first"
        );
        return nullptr;
    }

    clear_error();
    return env;
}

jint usagi_vm_attach_current_thread(
    UsagiVm *vm,
    JNIEnv **out_env
) {
    if (!valid_vm(vm) || !out_env) {
        set_error("invalid VM handle or output pointer");
        return JNI_ERR;
    }

    *out_env = nullptr;

    try {
        const jint status = vm->inner->GetJavaVM()
            ->AttachCurrentThread(out_env, nullptr);

        if (status != JNI_OK) {
            set_error("failed to attach current thread");
            return status;
        }

        clear_error();
        return JNI_OK;
    } catch (const std::exception &e) {
        set_error(e.what());
    } catch (...) {
        set_error("unexpected error attaching thread");
    }

    return JNI_ERR;
}

jint usagi_vm_detach_current_thread(UsagiVm *vm) {
    if (!valid_vm(vm)) {
        return JNI_ERR;
    }

    try {
        const jint status = vm->inner->GetJavaVM()
            ->DetachCurrentThread();

        if (status != JNI_OK) {
            set_error("failed to detach current thread");
            return status;
        }

        clear_error();
        return JNI_OK;
    } catch (const std::exception &e) {
        set_error(e.what());
    } catch (...) {
        set_error("unexpected error detaching thread");
    }

    return JNI_ERR;
}

jint usagi_vm_generate_class_dump(
    UsagiVm *vm,
    const char *path
) {
    if (!valid_vm(vm) || !path || path[0] == '\0') {
        set_error("invalid VM handle or dump path");
        return JNI_ERR;
    }

#ifdef JNI_DEBUG
    try {
        vm->inner->GenerateClassDump(path);
        clear_error();
        return JNI_OK;
    } catch (const std::exception &e) {
        set_error(e.what());
    } catch (...) {
        set_error("failed to generate class dump");
    }

    return JNI_ERR;
#else
    set_error("libjnivm was built without JNI_DEBUG");
    return JNI_ERR;
#endif
}

} // extern "C"
