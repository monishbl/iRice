// Sends the shell's requests to Hyprland's request socket, one per stdin line,
// and prints each answer followed by a record separator (0x1E). The C version
// of assets/scripts/glass-ipc.py (same protocol; the shell uses whichever is
// there, this one first): no interpreter to wake for every request.
//
// Why a separate process at all: Hyprland, once it accepts a connection,
// blocks its main loop (up to 5 s) until the request arrives. If the shell
// wrote from its own event loop while waiting on Hyprland itself (Qt does
// when it creates a window), both would wait on each other.
//
//     glass-ipc <socket path>
// Ends when stdin closes (the shell quit).
#include <errno.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

#define SEP '\x1e'

static int write_all(int fd, const char* p, size_t n) {
    while (n > 0) {
        ssize_t w = write(fd, p, n);
        if (w < 0) {
            if (errno == EINTR)
                continue;
            return -1;
        }
        p += w;
        n -= (size_t)w;
    }
    return 0;
}

// One request: connect, send, read the whole answer (3 s at most)
static void request(const char* path, char* cmd, size_t len) {
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    if (strlen(path) >= sizeof(addr.sun_path)) {
        fputs("error: socket path too long", stdout);
        return;
    }
    strcpy(addr.sun_path, path);
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0 || connect(fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        printf("error: %s", strerror(errno));
        if (fd >= 0)
            close(fd);
        return;
    }
    // Hyprland reads requests in 1023-byte chunks and waits for more when a
    // chunk is exactly full: never end a request on that edge
    if (len % 1023 == 0)
        cmd[len++] = ' ';
    if (write_all(fd, cmd, len) < 0) {
        printf("error: %s", strerror(errno));
        close(fd);
        return;
    }
    char buf[8192];
    struct pollfd pfd = {.fd = fd, .events = POLLIN};
    for (;;) {
        int r = poll(&pfd, 1, 3000);
        if (r <= 0) {
            if (r < 0 && errno == EINTR)
                continue;
            fputs(r == 0 ? "error: timed out" : "error: poll failed", stdout);
            break;
        }
        ssize_t n = read(fd, buf, sizeof(buf));
        if (n < 0 && errno == EINTR)
            continue;
        if (n <= 0)
            break;
        for (ssize_t i = 0; i < n; i++)
            if (buf[i] == SEP)
                buf[i] = ' ';
        fwrite(buf, 1, (size_t)n, stdout);
    }
    close(fd);
}

int main(int argc, char** argv) {
    if (argc < 2) {
        fputs("usage: glass-ipc <socket path>\n", stderr);
        return 2;
    }
    char*  line = NULL;
    size_t cap  = 0;
    ssize_t len;
    while ((len = getline(&line, &cap, stdin)) >= 0) {
        while (len > 0 && line[len - 1] == '\n')
            line[--len] = '\0';
        if (len == 0)
            continue;
        // (room for the padding space)
        if ((size_t)len + 2 > cap) {
            char* grown = realloc(line, (size_t)len + 2);
            if (!grown)
                return 1;
            line = grown;
            cap  = (size_t)len + 2;
        }
        request(argv[1], line, (size_t)len);
        putchar(SEP);
        if (fflush(stdout) == EOF)
            break; // the shell is gone
    }
    free(line);
    return 0;
}
