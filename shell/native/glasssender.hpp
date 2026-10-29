#pragma once
// ── GlassSender ─────────────────────────────────────────────────────────────
// One worker thread for the whole shell. Each window's newest request waits
// under its key; the worker sends whatever is waiting as one Hyprland batch,
// one request at a time, so an animation costs one request per frame however
// many windows move. Blocking socket calls happen only on this thread.
//
// No Qt in here, so it can be built and tested on its own
// (tests/test_native_glass.py).
//
// When a request can't reach Hyprland (no socket yet, a restart), it's tried
// again after 0.5 s, 1 s and 2 s, then given up: the next frame sends fresh
// shapes anyway, and an idle shell mustn't knock on a dead socket for ever.
// An answer that isn't "ok" (the plugin isn't loaded, a bad command) is
// final: sending the same thing again would get the same answer.
// On exit, whatever is still waiting (the windows' "clear my glass" lines)
// is sent once before the thread ends, without retries.

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cctype>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <unistd.h>

namespace irice {

class GlassSender {
public:
    static GlassSender& get() {
        static GlassSender s;
        return s;
    }

    // Path of Hyprland's request socket, from the environment
    static std::string socketPath() {
        const char* rt  = std::getenv("XDG_RUNTIME_DIR");
        const char* sig = std::getenv("HYPRLAND_INSTANCE_SIGNATURE");
        if (!sig)
            return {};
        return std::string(rt && *rt ? rt : "/tmp") + "/hypr/" + sig + "/.socket.sock";
    }

    explicit GlassSender(std::string path = socketPath()) : m_path(std::move(path)) {
        m_thread = std::thread([this] { run(); });
    }
    ~GlassSender() {
        {
            std::lock_guard lock(m_mutex);
            m_stop = true;
        }
        m_cv.notify_one();
        if (m_thread.joinable())
            m_thread.join();
    }
    GlassSender(const GlassSender&)            = delete;
    GlassSender& operator=(const GlassSender&) = delete;

    void submit(const std::string& key, std::string cmd) {
        {
            std::lock_guard lock(m_mutex);
            auto it = std::find_if(m_pending.begin(), m_pending.end(), [&](auto& p) { return p.first == key; });
            if (it != m_pending.end())
                it->second = std::move(cmd);
            else
                m_pending.emplace_back(key, std::move(cmd));
        }
        m_cv.notify_one();
    }

private:
    enum class Result { Ok, Refused, Unreachable };
    static constexpr int kBackoffMs[] = {500, 1000, 2000};

    void run() {
        int failures = 0; // in a row, for the current request
        for (;;) {
            std::vector<std::pair<std::string, std::string>> batch;
            bool                                             stopping;
            {
                std::unique_lock lock(m_mutex);
                m_cv.wait(lock, [this] { return m_stop || !m_pending.empty(); });
                if (m_pending.empty())
                    return; // stopping, nothing left
                stopping = m_stop;
                batch.swap(m_pending);
            }
            std::string cmd;
            if (batch.size() == 1) {
                cmd = batch[0].second;
            } else {
                cmd = "[[BATCH]]";
                for (size_t i = 0; i < batch.size(); i++)
                    cmd += (i ? ";" : "") + batch[i].second;
            }
            if (request(cmd, batch.size()) != Result::Unreachable || stopping || failures >= int(std::size(kBackoffMs))) {
                failures = 0;
                continue;
            }
            // Unreachable: put back what nothing newer has replaced, and wait
            std::unique_lock lock(m_mutex);
            for (auto& item : batch) {
                const auto newer = std::find_if(m_pending.begin(), m_pending.end(), [&](auto& p) { return p.first == item.first; });
                if (newer == m_pending.end())
                    m_pending.push_back(std::move(item));
            }
            m_cv.wait_for(lock, std::chrono::milliseconds(kBackoffMs[failures++]), [this] { return m_stop; });
        }
    }

    // Connect, send, read the answer to the end (3 s at most each way)
    Result request(std::string cmd, size_t expected) {
        if (m_path.empty())
            return Result::Ok; // no compositor in previews
        sockaddr_un addr{};
        addr.sun_family = AF_UNIX;
        if (m_path.size() >= sizeof(addr.sun_path))
            return Result::Refused;
        std::memcpy(addr.sun_path, m_path.c_str(), m_path.size() + 1);
        const int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
        if (fd < 0)
            return Result::Unreachable;
        timeval tv{3, 0};
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
        Result result = Result::Unreachable;
        if (connect(fd, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) == 0) {
            // Hyprland reads requests in 1023-byte chunks and waits for more
            // when a chunk is exactly full: never end on that edge
            if (cmd.size() % 1023 == 0)
                cmd += ' ';
            size_t sent = 0;
            while (sent < cmd.size()) {
                const ssize_t n = send(fd, cmd.data() + sent, cmd.size() - sent, MSG_NOSIGNAL);
                if (n < 0 && errno == EINTR)
                    continue;
                if (n <= 0)
                    break;
                sent += size_t(n);
            }
            std::string reply;
            bool        complete = false;
            if (sent == cmd.size()) {
                char buf[1024];
                for (;;) {
                    const ssize_t n = read(fd, buf, sizeof(buf));
                    if (n < 0 && errno == EINTR)
                        continue;
                    if (n == 0) {
                        complete = !reply.empty();
                        break;
                    }
                    if (n < 0 || reply.size() > 65536)
                        break;
                    reply.append(buf, size_t(n));
                }
            }
            if (complete) {
                // Each command answers "ok"; whitespace separates a batch's
                // answers. Anything else is Hyprland saying no.
                size_t acknowledgements = 0;
                bool   allOk            = true;
                for (size_t pos = 0; pos < reply.size();) {
                    if (std::isspace(static_cast<unsigned char>(reply[pos]))) {
                        ++pos;
                        continue;
                    }
                    if (reply.compare(pos, 2, "ok") != 0) {
                        allOk = false;
                        break;
                    }
                    pos += 2;
                    ++acknowledgements;
                }
                result = allOk && acknowledgements == expected ? Result::Ok : Result::Refused;
            }
        }
        close(fd);
        return result;
    }

    std::string                                      m_path;
    std::mutex                                       m_mutex;
    std::condition_variable                          m_cv;
    std::vector<std::pair<std::string, std::string>> m_pending;
    bool                                             m_stop = false;
    std::thread                                      m_thread;
};

} // namespace irice
