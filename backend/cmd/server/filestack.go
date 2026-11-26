package main

import (
	"context"
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/handler"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/jR4dh3y/BoxBox/backend/internal/websocket"
)

// fileStack is every service that reaches user files for one filesystem
// view. In NAS mode each mapped account gets its own, backed by its worker,
// so jobs, uploads and thumbnail caches never cross accounts.
type fileStack struct {
	jobs    service.JobService
	streams *handler.StreamHandler

	files            http.Handler
	emptyDirectories http.Handler
	stream           http.Handler
	search           http.Handler
	job              http.Handler
}

func newFileStack(fsys filesystem.FS, mountPoints []model.MountPoint, hub *websocket.Hub, cfg *model.ServerConfig, desktopMode bool, thumbnailLimit *service.ThumbnailLimit) (*fileStack, error) {
	fileConfig := service.FileServiceConfig{MountPoints: mountPoints}
	if desktopMode {
		fileConfig.DirectoryEntryLimit = -1
	}
	fileService := service.NewFileService(fsys, fileConfig)
	searchService := service.NewSearchService(fsys, service.SearchServiceConfig{MountPoints: mountPoints})
	jobService := service.NewJobService(fsys, hub, service.JobServiceConfig{
		Workers:     config.DefaultJobWorkers,
		MountPoints: mountPoints,
	})
	thumbnails := service.NewThumbnailService(fileService, thumbnailLimit)
	if desktopMode {
		var err error
		thumbnails, err = desktopThumbnails(fileService, thumbnailLimit)
		if err != nil {
			return nil, err
		}
	}
	streamHandler := handler.NewStreamHandlerWithThumbnails(fileService, cfg.MaxUploadMB, thumbnails)
	guarded := func(register func(chi.Router)) http.Handler {
		r := chi.NewRouter()
		r.Use(middleware.MountPointGuard(mountPoints))
		register(r)
		return r
	}
	plain := func(register func(chi.Router)) http.Handler {
		r := chi.NewRouter()
		register(r)
		return r
	}
	fileHandler := handler.NewFileHandler(fileService)
	return &fileStack{
		jobs:             jobService,
		streams:          streamHandler,
		files:            guarded(fileHandler.RegisterRoutes),
		emptyDirectories: guarded(fileHandler.RegisterEmptyDirectoryRoutes),
		stream:           guarded(streamHandler.RegisterRoutes),
		search:           plain(handler.NewSearchHandler(searchService).RegisterRoutes),
		job:              plain(handler.NewJobHandler(jobService).RegisterRoutes),
	}, nil
}

func (s *fileStack) start(ctx context.Context) {
	s.jobs.Start(ctx)
	s.streams.StartCleanup(ctx)
}

func (s *fileStack) stop() {
	s.jobs.Stop()
	s.streams.StopCleanup()
}

// fileStacks picks the stack for the signed-in account. Without NAS mode
// every account shares one stack.
type fileStacks struct {
	shared    *fileStack
	byAccount map[string]*fileStack
}

func (f fileStacks) all() []*fileStack {
	if f.byAccount == nil {
		return []*fileStack{f.shared}
	}
	stacks := make([]*fileStack, 0, len(f.byAccount))
	for _, stack := range f.byAccount {
		stacks = append(stacks, stack)
	}
	return stacks
}

func (f fileStacks) route(pick func(*fileStack) http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		stack := f.shared
		if f.byAccount != nil {
			stack = f.byAccount[authcontext.Username(r.Context())]
		}
		if stack == nil {
			handler.HandleServiceError(w, service.ErrNoFileAccess)
			return
		}
		pick(stack).ServeHTTP(w, r)
	})
}
