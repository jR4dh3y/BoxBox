// Package service provides business logic for the file manager.
// This file contains property-based tests for search operations.
package service

import (
	"context"
	"fmt"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/leanovate/gopter"
	"github.com/leanovate/gopter/gen"
	"github.com/leanovate/gopter/prop"
)

// setupTestSearchService creates a search service with an in-memory filesystem for testing
func setupTestSearchService() (SearchService, *filesystem.AferoFS) {
	fs := filesystem.NewMemMapFS()

	// Create mount point directories
	fs.MkdirAll("/data/media", 0755)
	fs.MkdirAll("/data/documents", 0755)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "documents", Path: "/data/documents", ReadOnly: false},
	}

	svc := NewSearchService(fs, SearchServiceConfig{MountPoints: mounts})
	return svc, fs
}

// **Feature: boxbox, Property 18: Search Result Correctness**
// **Validates: Requirements 9.1, 9.2**
//
// Property: For any search query Q in directory D, all returned results SHALL have
// names containing Q (case-insensitive) and paths prefixed by D.

func TestProperty_SearchResultCorrectness(t *testing.T) {
	parameters := gopter.DefaultTestParameters()
	parameters.MinSuccessfulTests = 100
	parameters.MaxSize = 50

	properties := gopter.NewProperties(parameters)

	// Generator for search query (non-empty alphanumeric)
	queryGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9]{0,10}`)

	// Generator for file names
	fileNameGen := gen.RegexMatch(`[a-zA-Z][a-zA-Z0-9_-]{0,15}`)

	// Generator for number of files
	numFilesGen := gen.IntRange(1, 20)

	properties.Property("all search results have names containing query (case-insensitive)", prop.ForAll(
		func(query string, fileNames []string, numFiles int) bool {
			if query == "" {
				return true // Skip empty queries
			}

			svc, fs := setupTestSearchService()
			ctx := context.Background()

			// Create test directory structure
			testDir := "/data/media/searchtest"
			fs.MkdirAll(testDir, 0755)
			fs.MkdirAll(testDir+"/subdir", 0755)

			// Create files - some matching, some not
			for i := 0; i < numFiles && i < len(fileNames); i++ {
				name := fileNames[i]
				if name == "" {
					continue
				}
				// Create file in root
				fs.WriteFile(testDir+"/"+name+".txt", []byte("content"), 0644)
				// Create file in subdir
				fs.WriteFile(testDir+"/subdir/"+name+"_sub.txt", []byte("content"), 0644)
			}

			// Also create some files that contain the query
			fs.WriteFile(testDir+"/"+query+"_match.txt", []byte("content"), 0644)
			fs.WriteFile(testDir+"/subdir/prefix_"+query+"_suffix.txt", []byte("content"), 0644)

			// Perform search
			results, err := svc.Search(ctx, "media/searchtest", query)
			if err != nil {
				return false
			}

			// Verify all results have names containing query (case-insensitive)
			queryLower := strings.ToLower(query)
			for _, result := range results {
				nameLower := strings.ToLower(result.Name)
				if !strings.Contains(nameLower, queryLower) {
					return false
				}
			}

			return true
		},
		queryGen,
		gen.SliceOfN(25, fileNameGen),
		numFilesGen,
	))

	properties.Property("all search results have paths prefixed by search directory", prop.ForAll(
		func(query string, subDirName string) bool {
			if query == "" || subDirName == "" {
				return true
			}

			svc, fs := setupTestSearchService()
			ctx := context.Background()

			// Create nested directory structure
			baseDir := "/data/media/" + subDirName
			fs.MkdirAll(baseDir, 0755)
			fs.MkdirAll(baseDir+"/level1", 0755)
			fs.MkdirAll(baseDir+"/level1/level2", 0755)

			// Create files with query in name at various levels
			fs.WriteFile(baseDir+"/"+query+"_root.txt", []byte("content"), 0644)
			fs.WriteFile(baseDir+"/level1/"+query+"_l1.txt", []byte("content"), 0644)
			fs.WriteFile(baseDir+"/level1/level2/"+query+"_l2.txt", []byte("content"), 0644)

			// Perform search from base directory
			searchPath := "media/" + subDirName
			results, err := svc.Search(ctx, searchPath, query)
			if err != nil {
				return false
			}

			// Verify all result paths are prefixed by the search directory
			for _, result := range results {
				if !strings.HasPrefix(result.Path, searchPath) {
					return false
				}
			}

			return true
		},
		queryGen,
		fileNameGen,
	))

	properties.Property("search is case-insensitive", prop.ForAll(
		func(query string) bool {
			if query == "" {
				return true
			}

			svc, fs := setupTestSearchService()
			ctx := context.Background()

			testDir := "/data/media/casetest"
			fs.MkdirAll(testDir, 0755)

			// Create files with different case variations
			upperQuery := strings.ToUpper(query)
			lowerQuery := strings.ToLower(query)
			mixedQuery := strings.Title(strings.ToLower(query))

			fs.WriteFile(testDir+"/"+upperQuery+"_upper.txt", []byte("content"), 0644)
			fs.WriteFile(testDir+"/"+lowerQuery+"_lower.txt", []byte("content"), 0644)
			fs.WriteFile(testDir+"/"+mixedQuery+"_mixed.txt", []byte("content"), 0644)

			// Search with original query
			results, err := svc.Search(ctx, "media/casetest", query)
			if err != nil {
				return false
			}

			// Should find all three files
			return len(results) >= 3
		},
		queryGen,
	))

	properties.Property("search finds files in subdirectories", prop.ForAll(
		func(query string, depth int) bool {
			if query == "" {
				return true
			}

			svc, fs := setupTestSearchService()
			ctx := context.Background()

			// Create nested directory structure with unique names that won't match query
			currentPath := "/data/media/deeptest"
			fs.MkdirAll(currentPath, 0755)

			// Create file at each level - use unique marker to identify our files
			createdFiles := make(map[string]bool)
			for i := 0; i <= depth; i++ {
				fileName := fmt.Sprintf("%s_level%d.txt", query, i)
				fs.WriteFile(currentPath+"/"+fileName, []byte("content"), 0644)
				createdFiles[fileName] = true

				if i < depth {
					// Use directory names that won't match the query
					currentPath = currentPath + fmt.Sprintf("/dir%d", i)
					fs.MkdirAll(currentPath, 0755)
				}
			}

			// Search from root
			results, err := svc.Search(ctx, "media/deeptest", query)
			if err != nil {
				return false
			}

			// Verify all created files are found in results
			foundFiles := 0
			for _, result := range results {
				if createdFiles[result.Name] {
					foundFiles++
				}
			}

			// Should find all files we created (may find more if query matches other things)
			return foundFiles == len(createdFiles)
		},
		queryGen,
		gen.IntRange(1, 5),
	))

	properties.Property("search returns empty results for non-matching query", prop.ForAll(
		func(fileNames []string, numFiles int) bool {
			svc, fs := setupTestSearchService()
			ctx := context.Background()

			testDir := "/data/media/nomatch"
			fs.MkdirAll(testDir, 0755)

			// Create files with predictable names (no 'xyz' substring)
			for i := 0; i < numFiles && i < len(fileNames); i++ {
				name := fileNames[i]
				if name == "" || strings.Contains(strings.ToLower(name), "xyz") {
					continue
				}
				fs.WriteFile(testDir+"/"+name+".txt", []byte("content"), 0644)
			}

			// Search for something that won't match
			results, err := svc.Search(ctx, "media/nomatch", "xyznonexistent")
			if err != nil {
				return false
			}

			// Should return empty results
			return len(results) == 0
		},
		gen.SliceOfN(20, fileNameGen),
		numFilesGen,
	))

	properties.Property("search results include complete metadata", prop.ForAll(
		func(query string) bool {
			if query == "" {
				return true
			}

			svc, fs := setupTestSearchService()
			ctx := context.Background()

			testDir := "/data/media/metadata"
			fs.MkdirAll(testDir, 0755)

			// Create a file and a directory matching the query
			fs.WriteFile(testDir+"/"+query+"_file.txt", []byte("test content"), 0644)
			fs.MkdirAll(testDir+"/"+query+"_dir", 0755)

			results, err := svc.Search(ctx, "media/metadata", query)
			if err != nil {
				return false
			}

			// Verify each result has complete metadata
			for _, result := range results {
				// Name must be non-empty
				if result.Name == "" {
					return false
				}

				// Path must be non-empty
				if result.Path == "" {
					return false
				}

				// Size must be non-negative
				if result.Size < 0 {
					return false
				}

				// ModTime must not be zero
				if result.ModTime.IsZero() {
					return false
				}
			}

			return true
		},
		queryGen,
	))

	properties.TestingRun(t)
}
