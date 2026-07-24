package handler

import (
	"encoding/json"
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

// JobHandler handles job-related HTTP requests
type JobHandler struct {
	jobService service.JobService
}

// NewJobHandler creates a new job handler
func NewJobHandler(jobService service.JobService) *JobHandler {
	return &JobHandler{
		jobService: jobService,
	}
}

// RegisterRoutes registers job routes on the given router
func (h *JobHandler) RegisterRoutes(r chi.Router) {
	r.Get("/", h.List)
	r.Post("/", h.Create)
	r.Get("/{id}", h.Get)
	r.Delete("/{id}", h.Cancel)
}

// CreateJobRequest represents the create job request body
type CreateJobRequest struct {
	Type       string `json:"type"`
	SourcePath string `json:"sourcePath"`
	DestPath   string `json:"destPath,omitempty"`
}

// JobResponse represents a job in API responses
type JobResponse struct {
	ID          string `json:"id"`
	Type        string `json:"type"`
	State       string `json:"state"`
	Progress    int    `json:"progress"`
	SourcePath  string `json:"sourcePath"`
	DestPath    string `json:"destPath,omitempty"`
	Error       string `json:"error,omitempty"`
	CreatedAt   string `json:"createdAt"`
	StartedAt   string `json:"startedAt,omitempty"`
	CompletedAt string `json:"completedAt,omitempty"`
}

// JobListResponse represents the list of jobs
type JobListResponse struct {
	Jobs []JobResponse `json:"jobs"`
}


// List returns all jobs
// GET /api/v1/jobs
func (h *JobHandler) List(w http.ResponseWriter, r *http.Request) {
	jobs, err := h.jobService.List(r.Context())
	if err != nil {
		writeError(w, "Failed to list jobs", model.ErrCodeInternalError, http.StatusInternalServerError)
		return
	}

	response := JobListResponse{
		Jobs: make([]JobResponse, len(jobs)),
	}

	for i, job := range jobs {
		response.Jobs[i] = h.toJobResponse(job)
	}

	writeJSON(w, response, http.StatusOK)
}

// Get returns a job by ID
// GET /api/v1/jobs/:id
func (h *JobHandler) Get(w http.ResponseWriter, r *http.Request) {
	jobID := chi.URLParam(r, "id")
	if jobID == "" {
		writeError(w, "Job ID is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	job, err := h.jobService.Get(r.Context(), jobID)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, h.toJobResponse(job), http.StatusOK)
}

// Create creates a new job
// POST /api/v1/jobs
func (h *JobHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req CreateJobRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, "Invalid request body", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Validate job type
	jobType := model.JobType(req.Type)
	if !jobType.IsValid() {
		writeError(w, "Invalid job type. Must be 'copy', 'move', or 'delete'", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Validate source path
	if req.SourcePath == "" {
		writeError(w, "Source path is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Validate destination path for copy and move
	if (jobType == model.JobTypeCopy || jobType == model.JobTypeMove) && req.DestPath == "" {
		writeError(w, "Destination path is required for copy and move operations", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	// Create job params
	params := model.JobParams{
		Type:       jobType,
		SourcePath: req.SourcePath,
		DestPath:   req.DestPath,
	}

	// Create job
	job, err := h.jobService.Create(r.Context(), params)
	if err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, h.toJobResponse(job), http.StatusAccepted)
}

// Cancel cancels a running job
// DELETE /api/v1/jobs/:id
func (h *JobHandler) Cancel(w http.ResponseWriter, r *http.Request) {
	jobID := chi.URLParam(r, "id")
	if jobID == "" {
		writeError(w, "Job ID is required", model.ErrCodeValidationError, http.StatusBadRequest)
		return
	}

	if err := h.jobService.Cancel(r.Context(), jobID); err != nil {
		HandleServiceError(w, err)
		return
	}

	writeJSON(w, map[string]string{"message": "Job cancelled successfully"}, http.StatusOK)
}

// toJobResponse converts a model.Job to JobResponse
func (h *JobHandler) toJobResponse(job *model.Job) JobResponse {
	resp := JobResponse{
		ID:         job.ID,
		Type:       string(job.Type),
		State:      string(job.State),
		Progress:   job.Progress,
		SourcePath: job.SourcePath,
		DestPath:   job.DestPath,
		Error:      job.Error,
		CreatedAt:  job.CreatedAt.Format("2006-01-02T15:04:05Z07:00"),
	}

	if !job.StartedAt.IsZero() {
		resp.StartedAt = job.StartedAt.Format("2006-01-02T15:04:05Z07:00")
	}

	if !job.CompletedAt.IsZero() {
		resp.CompletedAt = job.CompletedAt.Format("2006-01-02T15:04:05Z07:00")
	}

	return resp
}


