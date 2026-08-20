#ifndef EASYTIER_IOS_H
#define EASYTIER_IOS_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Initialize logger for iOS
 * path: log file path (can be empty for os_log only)
 * level: log level (trace, debug, info, warn, error)
 * subsystem: os_log subsystem identifier
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int init_logger(const char* path, const char* level, const char* subsystem, const char** err);

/* Clear logger */
int clear_logger(const char** err);

/* Start network instance with JSON configuration
 * cfg_str: JSON configuration string
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int run_network_instance(const char* cfg_str, const char** err);

/* Stop network instance
 * Returns: 0 on success */
int stop_network_instance(void);

/* Inject TUN file descriptor into running instance
 * fd: file descriptor from NEPacketTunnelProvider
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int set_tun_fd(int fd, const char** err);

/* Register stop callback
 * cb: callback function (can be NULL to unregister)
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int register_stop_callback(void (*cb)(void), const char** err);

/* Register running info callback
 * cb: callback function (can be NULL to unregister)
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int register_running_info_callback(void (*cb)(void), const char** err);

/* Get current running info as JSON
 * json: output JSON string (must be freed with free_string)
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int get_running_info(const char** json, const char** err);

/* Get latest error message
 * msg: output error message (must be freed with free_string)
 * err: output error message (must be freed with free_string)
 * Returns: 0 on success, -1 on error */
int get_latest_error_msg(const char** msg, const char** err);

/* Free string returned by this library */
void free_string(const char* s);

#ifdef __cplusplus
}
#endif

#endif /* EASYTIER_IOS_H */