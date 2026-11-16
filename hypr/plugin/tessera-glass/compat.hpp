// SPDX-License-Identifier: GPL-3.0-or-later
// Which Hyprland this plugin is built against, and the headers that moved.
//
// TESSERA_HYPRLAND_VERSION is major * 10000 + minor * 100 + patch (0.57.1:
// 5701), from `pkg-config --modversion hyprland` (the Makefile passes it).
// Built some other way: guessed from the headers (0.57 moved the window's).
// TG_057: Hyprland 0.57 or newer (render context passed explicitly, socket2
// and keybinds rewritten, workspaces and windows in folders of their own).
#pragma once

#ifndef TESSERA_HYPRLAND_VERSION
#if __has_include(<hyprland/src/desktop/view/window/Window.hpp>)
#define TESSERA_HYPRLAND_VERSION 5700
#else
#define TESSERA_HYPRLAND_VERSION 5600
#endif
#endif
#define TG_057 (TESSERA_HYPRLAND_VERSION >= 5700)

#if TG_057
#include <hyprland/src/desktop/view/window/Window.hpp>
#include <hyprland/src/desktop/view/window/WindowPresentation.hpp>
#include <hyprland/src/workspace/HLWorkspace.hpp>
#include <hyprland/src/ipc/s2/S2.hpp>
#include <hyprland/src/config/shared/actions/ConfigActions.hpp>
#else
#include <hyprland/src/desktop/view/Window.hpp>
#include <hyprland/src/desktop/Workspace.hpp>
#include <hyprland/src/managers/EventManager.hpp>
#include <hyprland/src/managers/KeybindManager.hpp>
#endif
#include <string>

// A layer surface is mapped (0.57 made the flag private)
#if TG_057
#define TG_LS_MAPPED(ls) (ls)->mapped()
#else
#define TG_LS_MAPPED(ls) (ls)->m_mapped
#endif

// A socket2 event (`name>>data`) for the shell
inline void tgPostEvent(const std::string& name, const std::string& data) {
#if TG_057
    IPC::Socket2::sock()->postEvent({name, data});
#else
    g_pEventManager->postEvent(SHyprIPCEvent{name, data});
#endif
}
