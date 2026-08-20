#ifndef KERN_CONTROL_H
#define KERN_CONTROL_H

#include <sys/socket.h>
#include <sys/sys_domain.h>
#include <net/if.h>

/* Kernel control structures for utun interface detection */

#define SYSPROTO_CONTROL 2
#define AF_SYS_CONTROL AF_SYSTEM

#define CTLIOCGINFO _IOR('N', 3, struct ctl_info)

struct ctl_info {
    u_int32_t ctl_id;
    char ctl_name[96];
};

struct sockaddr_ctl {
    u_char sc_len;
    u_char sc_family;
    u_int16_t ss_sysaddr;
    u_int32_t sc_id;
    u_int32_t sc_unit;
    u_int32_t sc_reserved[5];
};

#define UTUN_CONTROL_NAME "com.apple.net.utun_control"
#define UTUN_OPT_IFNAME 2

#endif /* KERN_CONTROL_H */