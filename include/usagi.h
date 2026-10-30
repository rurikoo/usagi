#ifndef USAGI_H
#define USAGI_H

#include <jni.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UsagiVm UsagiVm;
typedef struct UsagiObject UsagiObject;

enum {
    USAGI_OK = 0,
    USAGI_ERROR = -1,
    USAGI_BUFFER_TOO_SMALL = -2
};

/* create and destroy a VM */
UsagiVm *usagi_vm_create(void);
void usagi_vm_destroy(UsagiVm *vm);

/* access the JNI interfaces */
JavaVM *usagi_vm_get_java_vm(UsagiVm *vm);
JNIEnv *usagi_vm_get_jni_env(UsagiVm *vm);

/* thread lifecycle */
jint usagi_vm_attach_current_thread(
    UsagiVm *vm,
    JNIEnv **out_env
);

jint usagi_vm_detach_current_thread(UsagiVm *vm);

/* generate diagnostic stubs when libjnivm debug flag is enabled */
jint usagi_vm_generate_class_dump(
    UsagiVm *vm,
    const char *path
);

/* class lookup returns an owned object handle */
UsagiObject *usagi_vm_find_class(
    UsagiVm *vm,
    const char *internal_name
);

UsagiObject *usagi_vm_new_string_utf16(
    UsagiVm *vm,
    const uint16_t *units,
    size_t length
);

/* releases the reference and the handle */
void usagi_object_destroy(UsagiObject *object);

jint usagi_string_utf16_length(
    const UsagiObject *object,
    size_t *out_length
);

jint usagi_string_copy_utf16(
    const UsagiObject *object,
    uint16_t *buffer,
    size_t capacity,
    size_t *out_length
);

/* error from the last failed call on this thread */
const char *usagi_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* USAGI_H */
