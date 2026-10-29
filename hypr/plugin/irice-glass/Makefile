# Builds irice-glass.so against the installed Hyprland headers (pkg-config hyprland).
CXX ?= g++
CXXFLAGS ?= -O2
CXXFLAGS += -shared -fPIC -std=c++2b -Wno-c++11-narrowing
ifeq ($(CXX),g++)
    CXXFLAGS += --no-gnu-unique
endif
INCLUDES = `pkg-config --cflags pixman-1 libdrm hyprland libinput libudev wayland-server xkbcommon`
LIBS = -lGLESv2

# Built beside it, then swapped in: a running Hyprland that has the old one
# loaded keeps its copy (overwriting a loaded library in place crashes it)
irice-glass.so: main.cpp controls.cpp pointer.cpp pointer-policy.hpp focus-policy.hpp
	$(CXX) $(CXXFLAGS) $(INCLUDES) main.cpp controls.cpp pointer.cpp -o $@.new $(LIBS)
	mv -f $@.new $@

clean:
	rm -f irice-glass.so

.PHONY: clean
