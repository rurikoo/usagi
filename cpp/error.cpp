#include "internal.hpp"

namespace usagi {

namespace {
thread_local std::string last_error;
} // namespace

void clear_error() noexcept { last_error.clear(); }

void set_error(const char *message) noexcept {
  try {
    last_error = message ? message : "unknown native error";
  } catch (...) {
    last_error.clear();
  }
}

const std::string &current_error() noexcept { return last_error; }

bool valid_vm(const UsagiVm *vm) noexcept {
  if (!vm || !vm->inner) {
    set_error("invalid UsagiVm handle");
    return false;
  }
  return true;
}

bool valid_object(const UsagiObject *object) noexcept {
  if (!object || !object->owner || !object->global_ref) {
    set_error("invalid UsagiObject handle");
    return false;
  }
  return true;
}

bool valid_object_kind(int kind) noexcept {
  return kind == USAGI_OBJECT_KIND_OBJECT || kind == USAGI_OBJECT_KIND_CLASS ||
         kind == USAGI_OBJECT_KIND_STRING || kind == USAGI_OBJECT_KIND_ARRAY;
}

bool valid_primitive_array_kind(int kind) noexcept {
  return kind >= USAGI_ARRAY_KIND_BOOLEAN && kind <= USAGI_ARRAY_KIND_DOUBLE;
}

bool take_exception(JNIEnv *env) noexcept {
  if (!env) {
    return false;
  }

  try {
    if (env->ExceptionCheck()) {
      env->ExceptionClear();
      return true;
    }
  } catch (...) {
  }

  return false;
}

} // namespace usagi

extern "C" {

void usagi_set_error(const char *message) { usagi::set_error(message); }

const char *usagi_last_error(void) { return usagi::current_error().c_str(); }

} // extern "C"
