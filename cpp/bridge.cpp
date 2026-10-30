#include "usagi.h"

#include <jnivm.h>

#include <cstdint>
#include <exception>
#include <limits>
#include <memory>
#include <string>

struct UsagiVm {
  std::shared_ptr<jnivm::VM> inner;
};

enum class UsagiObjectKind {
  Object,
  Class,
  String,
};

struct UsagiObject {
  std::shared_ptr<jnivm::VM> owner;
  jobject global_ref = nullptr;
  UsagiObjectKind kind = UsagiObjectKind::Object;
};

namespace {

thread_local std::string last_error;

void clear_error() noexcept { last_error.clear(); }

void set_error(const char *message) noexcept {
  try {
    last_error = message ? message : "unknown native error";
  } catch (...) {
    last_error.clear();
  }
}

bool valid_vm(const UsagiVm *vm) noexcept {
  if (!vm || !vm->inner) {
    set_error("invalid UsagiVm handle");
    return false;
  }

  return true;
}

JNIEnv *current_env(const std::shared_ptr<jnivm::VM> &vm) {
  if (!vm) {
    set_error("invalid native VM");
    return nullptr;
  }

  JavaVM *java_vm = vm->GetJavaVM();

  if (!java_vm) {
    set_error("JavaVM unavailable");
    return nullptr;
  }

  JNIEnv *env = nullptr;

  const jint status =
      java_vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6);

  if (status != JNI_OK || !env) {
    set_error("JNI environment unavailable");
    return nullptr;
  }

  return env;
}

JNIEnv *current_env(UsagiVm *vm) {
  if (!valid_vm(vm)) {
    return nullptr;
  }

  return current_env(vm->inner);
}

struct LocalRefGuard {
  JNIEnv *env;
  jobject ref;

  ~LocalRefGuard() noexcept {
    if (!env || !ref) {
      return;
    }

    try {
      env->DeleteLocalRef(ref);
    } catch (...) {
    }
  }
};

struct StringCharsGuard {
  JNIEnv *env;
  jstring string;
  const jchar *chars;

  ~StringCharsGuard() noexcept {
    if (!env || !string || !chars) {
      return;
    }

    try {
      env->ReleaseStringChars(string, chars);
    } catch (...) {
    }
  }
};

UsagiObject *make_object_handle(UsagiVm *vm, JNIEnv *env, jobject local_ref,
                                UsagiObjectKind kind) {
  if (!valid_vm(vm) || !env || !local_ref) {
    set_error("invalid VM, JNI environment, or object reference");
    return nullptr;
  }

  auto handle = std::make_unique<UsagiObject>();

  jobject global_ref = env->NewGlobalRef(local_ref);

  if (!global_ref) {
    set_error("failed to create global JNI reference");
    return nullptr;
  }

  handle->owner = vm->inner;
  handle->global_ref = global_ref;
  handle->kind = kind;

  return handle.release();
}

bool valid_object(const UsagiObject *object) noexcept {
  if (!object || !object->owner || !object->global_ref) {
    set_error("invalid UsagiObject handle");
    return false;
  }

  return true;
}

} // namespace

extern "C" {

const char *usagi_last_error(void) { return last_error.c_str(); }

UsagiVm *usagi_vm_create(void) {
  clear_error();

  try {
    auto handle = std::make_unique<UsagiVm>();
    handle->inner = std::make_shared<jnivm::VM>();

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
  clear_error();
  delete vm;
}

JavaVM *usagi_vm_get_java_vm(UsagiVm *vm) {
  clear_error();

  try {
    if (!valid_vm(vm)) {
      return nullptr;
    }

    JavaVM *java_vm = vm->inner->GetJavaVM();

    if (!java_vm) {
      set_error("JavaVM unavailable");
      return nullptr;
    }

    clear_error();
    return java_vm;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("failed to get JavaVM");
  }

  return nullptr;
}

JNIEnv *usagi_vm_get_jni_env(UsagiVm *vm) {
  clear_error();

  try {
    JNIEnv *env = current_env(vm);

    if (!env) {
      return nullptr;
    }

    clear_error();
    return env;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("failed to get JNIEnv");
  }

  return nullptr;
}

jint usagi_vm_attach_current_thread(UsagiVm *vm, JNIEnv **out_env) {
  clear_error();

  if (out_env) {
    *out_env = nullptr;
  }

  try {
    if (!valid_vm(vm) || !out_env) {
      set_error("invalid VM or output pointer");
      return JNI_ERR;
    }

    JavaVM *java_vm = vm->inner->GetJavaVM();

    if (!java_vm) {
      set_error("JavaVM unavailable");
      return JNI_ERR;
    }

    const jint status = java_vm->AttachCurrentThread(out_env, nullptr);

    if (status != JNI_OK || !*out_env) {
      set_error("failed to attach current thread");
      return status == JNI_OK ? JNI_ERR : status;
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
  clear_error();

  try {
    if (!valid_vm(vm)) {
      return JNI_ERR;
    }

    JavaVM *java_vm = vm->inner->GetJavaVM();

    if (!java_vm) {
      set_error("JavaVM unavailable");
      return JNI_ERR;
    }

    const jint status = java_vm->DetachCurrentThread();

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

jint usagi_vm_generate_class_dump(UsagiVm *vm, const char *path) {
  clear_error();

  try {
    if (!valid_vm(vm) || !path || path[0] == '\0') {
      set_error("invalid VM or dump path");
      return JNI_ERR;
    }

#ifdef JNI_DEBUG
    vm->inner->GenerateClassDump(path);
    clear_error();
    return JNI_OK;
#else
    set_error("libjnivm was built without JNI_DEBUG");
    return JNI_ERR;
#endif
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("failed to generate class dump");
  }

  return JNI_ERR;
}

UsagiObject *usagi_vm_find_class(UsagiVm *vm, const char *internal_name) {
  clear_error();

  try {
    if (!valid_vm(vm) || !internal_name || internal_name[0] == '\0') {
      set_error("invalid VM or class name");
      return nullptr;
    }

    JNIEnv *env = current_env(vm);

    if (!env) {
      return nullptr;
    }

    jclass local = env->FindClass(internal_name);

    if (!local) {
      set_error("FindClass returned null");
      return nullptr;
    }

    LocalRefGuard guard{env, local};

    UsagiObject *object =
        make_object_handle(vm, env, local, UsagiObjectKind::Class);

    if (object) {
      clear_error();
    }

    return object;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("unexpected exception during class lookup");
  }

  return nullptr;
}

UsagiObject *usagi_vm_new_string_utf16(UsagiVm *vm, const uint16_t *units,
                                       size_t length) {
  clear_error();

  try {
    if (!valid_vm(vm)) {
      return nullptr;
    }

    if (length > static_cast<size_t>(std::numeric_limits<jsize>::max())) {
      set_error("string exceeds maximum JNI length");
      return nullptr;
    }

    if (length != 0 && !units) {
      set_error("null UTF-16 buffer with nonzero length");
      return nullptr;
    }

    JNIEnv *env = current_env(vm);

    if (!env) {
      return nullptr;
    }

    static_assert(sizeof(jchar) == sizeof(uint16_t));

    const jchar empty = 0;
    const jchar *chars =
        length == 0 ? &empty : reinterpret_cast<const jchar *>(units);

    jstring local = env->NewString(chars, static_cast<jsize>(length));

    if (!local) {
      set_error("NewString returned null");
      return nullptr;
    }

    LocalRefGuard guard{env, local};

    UsagiObject *object =
        make_object_handle(vm, env, local, UsagiObjectKind::String);

    if (object) {
      clear_error();
    }

    return object;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("unexpected exception creating Java string");
  }

  return nullptr;
}

void usagi_object_destroy(UsagiObject *object) {
  if (!object) {
    return;
  }

  clear_error();

  try {
    if (object->owner && object->global_ref) {
      JNIEnv *env = current_env(object->owner);

      if (env) {
        env->DeleteGlobalRef(object->global_ref);
        object->global_ref = nullptr;
      } else {
        set_error("could not obtain JNIEnv to release global reference");
      }
    }
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("unexpected exception destroying object handle");
  }

  delete object;
}

jint usagi_string_utf16_length(const UsagiObject *object, size_t *out_length) {
  clear_error();

  try {
    if (!valid_object(object) || !out_length) {
      set_error("invalid object or output pointer");
      return USAGI_ERROR;
    }

    if (object->kind != UsagiObjectKind::String) {
      set_error("object is not a Java string");
      return USAGI_ERROR;
    }

    JNIEnv *env = current_env(object->owner);

    if (!env) {
      return USAGI_ERROR;
    }

    const jsize length =
        env->GetStringLength(static_cast<jstring>(object->global_ref));

    if (length < 0) {
      set_error("failed to obtain Java string length");
      return USAGI_ERROR;
    }

    *out_length = static_cast<size_t>(length);
    clear_error();
    return USAGI_OK;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("unexpected exception obtaining string length");
  }

  return USAGI_ERROR;
}

jint usagi_string_copy_utf16(const UsagiObject *object, uint16_t *buffer,
                             size_t capacity, size_t *out_length) {
  clear_error();

  try {
    if (!valid_object(object) || !out_length) {
      set_error("invalid object or output pointer");
      return USAGI_ERROR;
    }

    if (object->kind != UsagiObjectKind::String) {
      set_error("object is not a Java string");
      return USAGI_ERROR;
    }

    JNIEnv *env = current_env(object->owner);

    if (!env) {
      return USAGI_ERROR;
    }

    jstring string = static_cast<jstring>(object->global_ref);

    const jsize length = env->GetStringLength(string);

    if (length < 0) {
      set_error("failed to obtain Java string length");
      return USAGI_ERROR;
    }

    const size_t required = static_cast<size_t>(length);
    *out_length = required;

    if (capacity < required) {
      set_error("UTF-16 output buffer is too small");
      return USAGI_BUFFER_TOO_SMALL;
    }

    if (required != 0 && !buffer) {
      set_error("null output buffer");
      return USAGI_ERROR;
    }

    const jchar *chars = env->GetStringChars(string, nullptr);

    StringCharsGuard guard{env, string, chars};

    if (!chars && required != 0) {
      set_error("failed to access Java string characters");
      return USAGI_ERROR;
    }

    for (size_t i = 0; i < required; ++i) {
      buffer[i] = static_cast<uint16_t>(chars[i]);
    }

    clear_error();
    return USAGI_OK;
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error("unexpected exception copying Java string");
  }

  return USAGI_ERROR;
}

} // extern "C"
