#ifndef USAGI_H
#define USAGI_H

#include <jni.h>

#ifdef __cplusplus
extern "C" {
#endif

/* opaque handle */
typedef struct UsagiVm UsagiVm;

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

/* error from the last failed call on this thread */
const char *usagi_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* USAGI_H */
