package engine

import (
	"log"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
)

// Watcher monitors file changes and triggers reloads
type Watcher struct {
	cfg      *Config
	loaders  *Loaders
	watcher  *fsnotify.Watcher
	debounce map[string]time.Time
	mu       sync.Mutex
	stopChan chan struct{}
}

// NewWatcher creates a new file watcher
func NewWatcher(cfg *Config, loaders *Loaders) (*Watcher, error) {
	fsWatcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}

	return &Watcher{
		cfg:      cfg,
		loaders:  loaders,
		watcher:  fsWatcher,
		debounce: make(map[string]time.Time),
		stopChan: make(chan struct{}),
	}, nil
}

// Start begins watching the configured directories
func (w *Watcher) Start() error {
	dirs := []string{
		w.cfg.Paths.SkillsDir,
		w.cfg.Paths.AgentsDir,
		w.cfg.Paths.RulesDir,
	}

	for _, dir := range dirs {
		if err := w.addWatch(dir); err != nil {
			log.Printf("Warning: failed to watch %s: %v", dir, err)
		}
	}

	go w.loop()

	return nil
}

// addWatch adds a directory to the watcher
func (w *Watcher) addWatch(dir string) error {
	// Add the directory itself
	if err := w.watcher.Add(dir); err != nil {
		return err
	}

	// Add subdirectories for skills
	if dir == w.cfg.Paths.SkillsDir {
		entries, err := filepath.Glob(filepath.Join(dir, "*"))
		if err != nil {
			return err
		}
		for _, entry := range entries {
			if isDir(entry) {
				w.watcher.Add(entry)
				// Watch subdirectories like scripts/, references/
				subs, _ := filepath.Glob(filepath.Join(entry, "*"))
				for _, sub := range subs {
					if isDir(sub) {
						w.watcher.Add(sub)
					}
				}
			}
		}
	}

	return nil
}

// Stop stops the watcher
func (w *Watcher) Stop() {
	close(w.stopChan)
	w.watcher.Close()
}

// loop processes file system events
func (w *Watcher) loop() {
	debounceDelay := 500 * time.Millisecond

	for {
		select {
		case <-w.stopChan:
			return
		case event, ok := <-w.watcher.Events:
			if !ok {
				return
			}

			// Debounce rapid file changes
			w.mu.Lock()
			lastTime, exists := w.debounce[event.Name]
			now := time.Now()
			w.debounce[event.Name] = now
			w.mu.Unlock()

			if exists && now.Sub(lastTime) < debounceDelay {
				continue
			}

			if event.Has(fsnotify.Write) || event.Has(fsnotify.Create) || event.Has(fsnotify.Remove) {
				w.handleEvent(event.Name)
			}

		case err, ok := <-w.watcher.Errors:
			if !ok {
				return
			}
			log.Printf("Watcher error: %v", err)
		}
	}
}

// handleEvent processes a single file system event
func (w *Watcher) handleEvent(path string) {
	// Determine which directory was affected
	skillsDir := w.cfg.Paths.SkillsDir
	agentsDir := w.cfg.Paths.AgentsDir
	rulesDir := w.cfg.Paths.RulesDir

	if stringsContains(path, skillsDir) {
		log.Printf("Skills changed, reloading...")
		w.loaders.LoadSkills()
	} else if stringsContains(path, agentsDir) {
		log.Printf("Agents changed, reloading...")
		w.loaders.LoadAgents()
	} else if stringsContains(path, rulesDir) {
		log.Printf("Rules changed, reloading...")
		w.loaders.LoadRules()
	}
}

func isDir(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func stringsContains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || filepath.Dir(s) == substr ||
		len(s) > len(substr) && s[:len(substr)] == substr)
}
