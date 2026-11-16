# Builds tessera-glass.so against the installed Hyprland headers (pkg-config hyprland).
# (another Hyprland's: PKG_CONFIG_PATH pointing at its hyprland.pc)
CXX ?= g++
CXXFLAGS ?= -O2
CXXFLAGS += -shared -fPIC -std=c++2b -Wno-c++11-narrowing
ifeq ($(CXX),g++)
    CXXFLAGS += --no-gnu-unique
endif
INCLUDES = `pkg-config --cflags pixman-1 libdrm hyprland libinput libudev wayland-server xkbcommon`
# The Hyprland it's built for, as a number (0.57.1: 5701): compat.hpp
HYPRLAND_VERSION := $(shell pkg-config --modversion hyprland 2>/dev/null | awk -F. '{ printf "%d", $$1 * 10000 + $$2 * 100 + $$3 }')
ifneq ($(HYPRLAND_VERSION),)
    CXXFLAGS += -DTESSERA_HYPRLAND_VERSION=$(HYPRLAND_VERSION)
endif
LIBS = -lGLESv2
OUT ?= tessera-glass.so

# Built beside it, then swapped in: a running Hyprland that has the old one
# loaded keeps its copy (overwriting a loaded library in place crashes it)
$(OUT): main.cpp controls.cpp pointer.cpp hold.cpp pointer-policy.hpp focus-policy.hpp compat.hpp
	$(CXX) $(CXXFLAGS) $(INCLUDES) main.cpp controls.cpp pointer.cpp hold.cpp -o $@.new $(LIBS)
	mv -f $@.new $@

clean:
	rm -f tessera-glass.so

.PHONY: clean
