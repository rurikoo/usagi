#ifndef USAGI_INTERNAL_HPP
#define USAGI_INTERNAL_HPP

#include "usagi.h"

#include <jnivm.h>

#include <exception>
#include <memory>
#include <string>

struct UsagiVm {
  std::shared_ptr<jnivm::VM> inner;
};

struct UsagiObject {
  std::shared_ptr<jnivm::VM> owner;
  jobject global_ref = nullptr;
  UsagiObjectKind kind = USAGI_OBJECT_KIND_OBJECT;
  UsagiArrayKind array_kind = USAGI_ARRAY_KIND_NONE;
};

namespace usagi {

void clear_error() noexcept;
void set_error(const char *message) noexcept;
const std::string &current_error() noexcept;

bool valid_vm(const UsagiVm *vm) noexcept;
bool valid_object(const UsagiObject *object) noexcept;
bool valid_object_kind(int kind) noexcept;
bool valid_primitive_array_kind(int kind) noexcept;

JNIEnv *current_env(const std::shared_ptr<jnivm::VM> &vm);
JNIEnv *current_env(UsagiVm *vm);

bool take_exception(JNIEnv *env) noexcept;

UsagiObject *make_object_handle(const std::shared_ptr<jnivm::VM> &vm,
                                JNIEnv *env, jobject ref,
                                UsagiObjectKind expected_kind);

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

template <class Fn>
jint catch_to_status(Fn &&fn, const char *fallback,
                     jint err_code = USAGI_ERROR) noexcept {
  clear_error();
  try {
    return fn();
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error(fallback);
  }
  return err_code;
}

template <class T, class Fn>
T *catch_to_ptr(Fn &&fn, const char *fallback) noexcept {
  clear_error();
  try {
    return fn();
  } catch (const std::exception &e) {
    set_error(e.what());
  } catch (...) {
    set_error(fallback);
  }
  return nullptr;
}

} // namespace usagi

#endif /* USAGI_INTERNAL_HPP */
