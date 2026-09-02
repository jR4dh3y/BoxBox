package model

import "time"

// JobType represents the type of background job
type JobType string

const (
	JobTypeCopy   JobType = "copy"
	JobTypeMove   JobType = "move"
	JobTypeDelete JobType = "delete"
)

// JobState represents the current state of a job
type JobState string

const (
	JobStatePending   JobState = "pending"
	JobStateRunning   JobState = "running"
	JobStateCompleted JobState = "completed"
	JobStateFailed    JobState = "failed"
	JobStateCancelled JobState = "cancelled"
)

// Job represents a background job for file operations
type Job struct {
	ID                 string    `json:"id"`
	Owner              string    `json:"-"`
	Type               JobType   `json:"type"`
	State              JobState  `json:"state"`
	Progress           int       `json:"progress"` // 0-100
	SourcePath         string    `json:"sourcePath"`
	DestPath           string    `json:"destPath,omitempty"`
	ResolvedSourcePath string    `json:"-"`
	ResolvedDestPath   string    `json:"-"`
	ResolvedSourceRoot string    `json:"-"`
	ResolvedDestRoot   string    `json:"-"`
	Error              string    `json:"error,omitempty"`
	CreatedAt          time.Time `json:"createdAt"`
	StartedAt          time.Time `json:"startedAt,omitzero"`
	CompletedAt        time.Time `json:"completedAt,omitzero"`
}

// JobUpdate represents a progress update for a job sent via WebSocket
type JobUpdate struct {
	JobID    string   `json:"jobId"`
	State    JobState `json:"state"`
	Progress int      `json:"progress"`
	Error    string   `json:"error,omitempty"`
}

// JobParams contains parameters for creating a new job
type JobParams struct {
	Type       JobType `json:"type"`
	SourcePath string  `json:"sourcePath"`
	DestPath   string  `json:"destPath,omitempty"`
}

// IsTerminal returns true if the job state is a terminal state
func (s JobState) IsTerminal() bool {
	return s == JobStateCompleted || s == JobStateFailed || s == JobStateCancelled
}

// IsValid returns true if the job type is valid
func (t JobType) IsValid() bool {
	return t == JobTypeCopy || t == JobTypeMove || t == JobTypeDelete
}
