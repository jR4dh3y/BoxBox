// Package service provides business logic for the file manager.
// This file contains property-based tests for file operations.
package service

import (
	"context"
	"fmt"
	"testing"

	"github.com/homelab/filemanager/internal/model"
	"github.com/homelab/filemanager/internal/pkg/filesystem"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// setupTestFileService creates a file service with an in-memory filesystem for testing
func setupTestFileService() (FileService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()

	// Create mount point directories
	fs.MkdirAll("/data/media", 0755)
	fs.MkdirAll("/data/documents", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/data/documents", ReadOnly: false},
	}

	svc := NewFileService(fs, FileServiceConfig{MountPoints: mounts})
	return svc, fs
}

func TestDirectoryListingPutsVisibleItemsBeforeHiddenItems(t *testing.T) {
	svc, fs := setupTestFileService()
	ctx := context.Background()

	testDir := "/data/media/hidden-first"
	if err := fs.MkdirAll(testDir, 0755); err != nil {
		t.Fatal(err)
	}

	for i := 0; i < 20; i++ {
		if err := fs.WriteFile(fmt.Sprintf("%s/.hidden%02d", testDir, i), []byte("content"), 0644); err != nil {
			t.Fatal(err)
		}
	}
	for _, name := range []string{"Desktop", "Documents", "Downloads"} {
		if err := fs.MkdirAll(testDir+"/"+name, 0755); err != nil {
			t.Fatal(err)
		}
	}

	list, err := svc.List(ctx, "media/hidden-first", model.ListOptions{
		Page:     1,
		PageSize: 3,
		SortBy:   "name",
		SortDir:  "asc",
	})
	if err != nil {
		t.Fatal(err)
	}

	got := make([]string, 0, len(list.Items))
	for _, item := range list.Items {
		got = append(got, item.Name)
	}

	want := []string{"Desktop", "Documents", "Downloads"}
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Fatalf("expected first page to contain visible items %v, got %v", want, got)
	}
}

// **Feature: homelab-file-manager, Property 1: Directory Listing Metadata Completeness**
// **Validates: Requirements 1.1**
//
// Property: For any valid directory path, the listing response SHALL contain items
// where each item includes non-empty name, valid path, non-negative size, boolean isDir flag,
// and valid modTime timestamp.

func TestProperty_DirectoryListingMetadataCompleteness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file names (alphanumeric with some allowed chars)
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,20}`)

	// Generator for file extensions
	extGen := gen.OneConstOf(".txt", ".pdf", ".jpg", ".mp4", ".doc", "")

	// Generator for number of files to create
	numFilesGen := gen.IntRange(1, 20)

	properties.Property("directory listing items have complete metadata", prop.ForAll(
		func(fileNames []string, numFiles int) bool {
			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create test directory
			testDir := "/data/media/testdir"
			fs.MkdirAll(testDir, 0755)

			// Create files with unique names
			createdFiles := make(map[string]bool)
			for i := 0; i < numFiles && i < len(fileNames); i++ {
				name := fileNames[i]
				if name == "" || createdFiles[name] {
					continue
				}
				createdFiles[name] = true
				fs.WriteFile(testDir+"/"+name, []byte("content"), 0644)
			}

			// Also create a subdirectory
			fs.MkdirAll(testDir+"/subdir", 0755)

			// List the directory
			list, err := svc.List(ctx, "media/testdir", model.DefaultListOptions())
			if err != nil {
				return false
			}

			// Verify each item has complete metadata
			for _, item := range list.Items {
				// Name must be non-empty
				if item.Name == "" {
					return false
				}

				// Path must be non-empty and contain the name
				if item.Path == "" {
					return false
				}

				// Size must be non-negative
				if item.Size < 0 {
					return false
				}

				// ModTime must not be zero
				if item.ModTime.IsZero() {
					return false
				}

				// Permissions must be non-empty
				if item.Permissions == "" {
					return false
				}
			}

			return true
		},
		gen.SliceOfN(25, fileNameGen),
		numFilesGen,
	))

	properties.Property("file items have correct isDir flag", prop.ForAll(
		func(fileName string, ext string) bool {
			if fileName == "" {
				return true // Skip empty names
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			testDir := "/data/media/flagtest"
			fs.MkdirAll(testDir, 0755)

			// Create a file
			fullFileName := fileName + ext
			fs.WriteFile(testDir+"/"+fullFileName, []byte("content"), 0644)

			// Create a directory
			fs.MkdirAll(testDir+"/"+fileName+"_dir", 0755)

			// List and verify
			list, err := svc.List(ctx, "media/flagtest", model.DefaultListOptions())
			if err != nil {
				return false
			}

			for _, item := range list.Items {
				if item.Name == fullFileName && item.IsDir {
					return false // File should not be marked as directory
				}
				if item.Name == fileName+"_dir" && !item.IsDir {
					return false // Directory should be marked as directory
				}
			}

			return true
		},
		fileNameGen,
		extGen,
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 2: Pagination Correctness**
// **Validates: Requirements 1.3**
//
// Property: For any directory with N items and requested page size P,
// the returned items count SHALL be at most P, and the totalCount SHALL equal N
// regardless of page number.

func TestProperty_PaginationCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for number of files
	numFilesGen := gen.IntRange(0, 100)

	// Generator for page size
	pageSizeGen := gen.IntRange(1, 50)

	// Generator for page number
	pageNumGen := gen.IntRange(1, 10)

	properties.Property("returned items count is at most page size", prop.ForAll(
		func(numFiles int, pageSize int, pageNum int) bool {
			svc, fs := setupTestFileService()
			ctx := context.Background()

			testDir := "/data/media/pagination"
			fs.MkdirAll(testDir, 0755)

			// Create files
			for i := 0; i < numFiles; i++ {
				fs.WriteFile(fmt.Sprintf("%s/file%03d.txt", testDir, i), []byte("content"), 0644)
			}

			opts := model.ListOptions{
				Page:     pageNum,
				PageSize: pageSize,
				SortBy:   "name",
				SortDir:  "asc",
			}

			list, err := svc.List(ctx, "media/pagination", opts)
			if err != nil {
				return false
			}

			// Items count should be at most page size
			if len(list.Items) > pageSize {
				return false
			}

			return true
		},
		numFilesGen,
		pageSizeGen,
		pageNumGen,
	))

	properties.Property("totalCount equals actual number of items regardless of page", prop.ForAll(
		func(numFiles int, pageSize int, pageNum int) bool {
			svc, fs := setupTestFileService()
			ctx := context.Background()

			testDir := "/data/media/totalcount"
			fs.MkdirAll(testDir, 0755)

			// Create files
			for i := 0; i < numFiles; i++ {
				fs.WriteFile(fmt.Sprintf("%s/file%03d.txt", testDir, i), []byte("content"), 0644)
			}

			opts := model.ListOptions{
				Page:     pageNum,
				PageSize: pageSize,
				SortBy:   "name",
				SortDir:  "asc",
			}

			list, err := svc.List(ctx, "media/totalcount", opts)
			if err != nil {
				return false
			}

			// TotalCount should equal actual number of files
			if list.TotalCount != numFiles {
				return false
			}

			return true
		},
		numFilesGen,
		pageSizeGen,
		pageNumGen,
	))

	properties.Property("pagination covers all items without duplicates", prop.ForAll(
		func(numFiles int, pageSize int) bool {
			if numFiles == 0 || pageSize == 0 {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			testDir := "/data/media/nodupes"
			fs.MkdirAll(testDir, 0755)

			// Create files
			for i := 0; i < numFiles; i++ {
				fs.WriteFile(fmt.Sprintf("%s/file%03d.txt", testDir, i), []byte("content"), 0644)
			}

			// Collect all items across all pages
			allItems := make(map[string]bool)
			totalPages := (numFiles + pageSize - 1) / pageSize

			for page := 1; page <= totalPages; page++ {
				opts := model.ListOptions{
					Page:     page,
					PageSize: pageSize,
					SortBy:   "name",
					SortDir:  "asc",
				}

				list, err := svc.List(ctx, "media/nodupes", opts)
				if err != nil {
					return false
				}

				for _, item := range list.Items {
					if allItems[item.Name] {
						return false // Duplicate found
					}
					allItems[item.Name] = true
				}
			}

			// Should have collected all files
			return len(allItems) == numFiles
		},
		gen.IntRange(1, 50),
		gen.IntRange(1, 20),
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 3: Non-Existent Path Returns 404**
// **Validates: Requirements 1.5, 3.4**
//
// Property: For any path that does not exist in the filesystem,
// requesting that path SHALL return an error (ErrPathNotFound).

func TestProperty_NonExistentPathReturnsError(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for random path segments
	pathSegmentGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	// Generator for number of path segments
	numSegmentsGen := gen.IntRange(1, 5)

	properties.Property("non-existent paths return ErrPathNotFound for List", prop.ForAll(
		func(segments []string, numSegments int) bool {
			svc, _ := setupTestFileService()
			ctx := context.Background()

			// Build a random path that doesn't exist
			path := "media"
			for i := 0; i < numSegments && i < len(segments); i++ {
				if segments[i] != "" {
					path += "/" + segments[i]
				}
			}
			path += "/nonexistent_" + fmt.Sprintf("%d", numSegments)

			// Try to list the non-existent path
			_, err := svc.List(ctx, path, model.DefaultListOptions())

			// Should return ErrPathNotFound
			return err == ErrPathNotFound
		},
		gen.SliceOfN(10, pathSegmentGen),
		numSegmentsGen,
	))

	properties.Property("non-existent paths return ErrPathNotFound for GetInfo", prop.ForAll(
		func(segments []string, numSegments int) bool {
			svc, _ := setupTestFileService()
			ctx := context.Background()

			// Build a random path that doesn't exist
			path := "media"
			for i := 0; i < numSegments && i < len(segments); i++ {
				if segments[i] != "" {
					path += "/" + segments[i]
				}
			}
			path += "/nonexistent_file_" + fmt.Sprintf("%d", numSegments)

			// Try to get info for the non-existent path
			_, err := svc.GetInfo(ctx, path)

			// Should return ErrPathNotFound
			return err == ErrPathNotFound
		},
		gen.SliceOfN(10, pathSegmentGen),
		numSegmentsGen,
	))

	properties.Property("existing paths do not return ErrPathNotFound", prop.ForAll(
		func(dirName string) bool {
			if dirName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create the directory
			testDir := "/data/media/" + dirName
			fs.MkdirAll(testDir, 0755)

			// Try to list it
			_, err := svc.List(ctx, "media/"+dirName, model.DefaultListOptions())

			// Should NOT return ErrPathNotFound
			return err != ErrPathNotFound
		},
		pathSegmentGen,
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 15: File Rename Correctness**
// **Validates: Requirements 8.1**
//
// Property: For any successful rename operation from path A to path B,
// the file SHALL exist at path B with identical content and no longer exist at path A.

func TestProperty_FileRenameCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	// Generator for file content
	contentGen := gen.SliceOfN(100, gen.UInt8())

	properties.Property("renamed file exists at new path with identical content", prop.ForAll(
		func(oldName string, newName string, content []byte) bool {
			if oldName == "" || newName == "" || oldName == newName {
				return true // Skip invalid cases
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create the original file
			oldPath := "/data/media/" + oldName + ".txt"
			fs.WriteFile(oldPath, content, 0644)

			// Perform rename
			err := svc.Rename(ctx, "media/"+oldName+".txt", "media/"+newName+".txt")
			if err != nil {
				return false
			}

			// Verify new file exists with same content
			newPath := "/data/media/" + newName + ".txt"
			newContent, err := fs.ReadFile(newPath)
			if err != nil {
				return false
			}

			// Content should be identical
			if len(newContent) != len(content) {
				return false
			}
			for i := range content {
				if newContent[i] != content[i] {
					return false
				}
			}

			return true
		},
		fileNameGen,
		fileNameGen,
		contentGen,
	))

	properties.Property("renamed file no longer exists at old path", prop.ForAll(
		func(oldName string, newName string) bool {
			if oldName == "" || newName == "" || oldName == newName {
				return true // Skip invalid cases
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create the original file
			oldPath := "/data/media/" + oldName + ".txt"
			fs.WriteFile(oldPath, []byte("test content"), 0644)

			// Perform rename
			err := svc.Rename(ctx, "media/"+oldName+".txt", "media/"+newName+".txt")
			if err != nil {
				return false
			}

			// Verify old path no longer exists
			exists, _ := fs.Exists(oldPath)
			return !exists
		},
		fileNameGen,
		fileNameGen,
	))

	properties.Property("directory rename preserves all contents", prop.ForAll(
		func(oldDirName string, newDirName string, numFiles int) bool {
			if oldDirName == "" || newDirName == "" || oldDirName == newDirName {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create directory with files
			oldDir := "/data/media/" + oldDirName
			fs.MkdirAll(oldDir, 0755)

			fileContents := make(map[string][]byte)
			for i := 0; i < numFiles; i++ {
				fileName := fmt.Sprintf("file%d.txt", i)
				content := []byte(fmt.Sprintf("content %d", i))
				fs.WriteFile(oldDir+"/"+fileName, content, 0644)
				fileContents[fileName] = content
			}

			// Perform rename
			err := svc.Rename(ctx, "media/"+oldDirName, "media/"+newDirName)
			if err != nil {
				return false
			}

			// Verify all files exist in new location with same content
			newDir := "/data/media/" + newDirName
			for fileName, expectedContent := range fileContents {
				actualContent, err := fs.ReadFile(newDir + "/" + fileName)
				if err != nil {
					return false
				}
				if string(actualContent) != string(expectedContent) {
					return false
				}
			}

			// Verify old directory no longer exists
			exists, _ := fs.Exists(oldDir)
			return !exists
		},
		fileNameGen,
		fileNameGen,
		gen.IntRange(1, 10),
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 16: Directory Creation Correctness**
// **Validates: Requirements 8.2**
//
// Property: For any successful directory creation at path P,
// the path P SHALL exist and be a directory.

func TestProperty_DirectoryCreationCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for directory names
	dirNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	properties.Property("created directory exists", prop.ForAll(
		func(dirName string) bool {
			if dirName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create directory
			err := svc.CreateDir(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			// Verify it exists
			exists, err := fs.Exists("/data/media/" + dirName)
			if err != nil {
				return false
			}

			return exists
		},
		dirNameGen,
	))

	properties.Property("created path is a directory not a file", prop.ForAll(
		func(dirName string) bool {
			if dirName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create directory
			err := svc.CreateDir(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			// Verify it's a directory
			isDir, err := fs.IsDir("/data/media/" + dirName)
			if err != nil {
				return false
			}

			return isDir
		},
		dirNameGen,
	))

	properties.Property("nested directory creation works", prop.ForAll(
		func(parentName string, childName string) bool {
			if parentName == "" || childName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create parent directory first
			err := svc.CreateDir(ctx, "media/"+parentName)
			if err != nil {
				return false
			}

			// Create child directory
			err = svc.CreateDir(ctx, "media/"+parentName+"/"+childName)
			if err != nil {
				return false
			}

			// Verify both exist and are directories
			parentIsDir, _ := fs.IsDir("/data/media/" + parentName)
			childIsDir, _ := fs.IsDir("/data/media/" + parentName + "/" + childName)

			return parentIsDir && childIsDir
		},
		dirNameGen,
		dirNameGen,
	))

	properties.Property("GetInfo returns isDir=true for created directories", prop.ForAll(
		func(dirName string) bool {
			if dirName == "" {
				return true
			}

			svc, _ := setupTestFileService()
			ctx := context.Background()

			// Create directory
			err := svc.CreateDir(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			// Get info via service
			info, err := svc.GetInfo(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			return info.IsDir
		},
		dirNameGen,
	))

	properties.TestingRun(t)
}

// **Feature: homelab-file-manager, Property 17: File Deletion Correctness**
// **Validates: Requirements 8.3**
//
// Property: For any successful delete operation on path P,
// the path P SHALL no longer exist in the filesystem.

func TestProperty_FileDeletionCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	properties.Property("deleted file no longer exists", prop.ForAll(
		func(fileName string) bool {
			if fileName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create file
			filePath := "/data/media/" + fileName + ".txt"
			fs.WriteFile(filePath, []byte("content"), 0644)

			// Verify it exists
			exists, _ := fs.Exists(filePath)
			if !exists {
				return false
			}

			// Delete it
			err := svc.Delete(ctx, "media/"+fileName+".txt")
			if err != nil {
				return false
			}

			// Verify it no longer exists
			exists, _ = fs.Exists(filePath)
			return !exists
		},
		fileNameGen,
	))

	properties.Property("deleted directory no longer exists", prop.ForAll(
		func(dirName string) bool {
			if dirName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create directory
			dirPath := "/data/media/" + dirName
			fs.MkdirAll(dirPath, 0755)

			// Verify it exists
			exists, _ := fs.Exists(dirPath)
			if !exists {
				return false
			}

			// Delete it
			err := svc.Delete(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			// Verify it no longer exists
			exists, _ = fs.Exists(dirPath)
			return !exists
		},
		fileNameGen,
	))

	properties.Property("deleting directory removes all contents", prop.ForAll(
		func(dirName string, numFiles int) bool {
			if dirName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create directory with files
			dirPath := "/data/media/" + dirName
			fs.MkdirAll(dirPath, 0755)

			filePaths := make([]string, numFiles)
			for i := 0; i < numFiles; i++ {
				filePath := fmt.Sprintf("%s/file%d.txt", dirPath, i)
				fs.WriteFile(filePath, []byte("content"), 0644)
				filePaths[i] = filePath
			}

			// Delete directory
			err := svc.Delete(ctx, "media/"+dirName)
			if err != nil {
				return false
			}

			// Verify directory and all files no longer exist
			dirExists, _ := fs.Exists(dirPath)
			if dirExists {
				return false
			}

			for _, filePath := range filePaths {
				exists, _ := fs.Exists(filePath)
				if exists {
					return false
				}
			}

			return true
		},
		fileNameGen,
		gen.IntRange(1, 10),
	))

	properties.Property("GetInfo returns error for deleted paths", prop.ForAll(
		func(fileName string) bool {
			if fileName == "" {
				return true
			}

			svc, fs := setupTestFileService()
			ctx := context.Background()

			// Create and delete file
			filePath := "/data/media/" + fileName + ".txt"
			fs.WriteFile(filePath, []byte("content"), 0644)

			err := svc.Delete(ctx, "media/"+fileName+".txt")
			if err != nil {
				return false
			}

			// GetInfo should return error
			_, err = svc.GetInfo(ctx, "media/"+fileName+".txt")
			return err == ErrPathNotFound
		},
		fileNameGen,
	))

	properties.TestingRun(t)
}
