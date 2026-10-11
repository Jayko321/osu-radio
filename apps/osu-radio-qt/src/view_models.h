#pragma once
#include "src/qt_view_models.cxxqt.h"
#include <memory>
inline std::unique_ptr<LibraryViewModel> newLibraryViewModel() { return std::make_unique<LibraryViewModel>(); }
inline std::unique_ptr<PlaylistsViewModel> newPlaylistsViewModel() { return std::make_unique<PlaylistsViewModel>(); }
inline std::unique_ptr<SettingsViewModel> newSettingsViewModel() { return std::make_unique<SettingsViewModel>(); }
inline std::unique_ptr<PlayerViewModel> newPlayerViewModel() { return std::make_unique<PlayerViewModel>(); }
inline std::unique_ptr<QueueViewModel> newQueueViewModel() { return std::make_unique<QueueViewModel>(); }
inline std::unique_ptr<TrackListModel> newTrackListModel() { return std::make_unique<TrackListModel>(); }
