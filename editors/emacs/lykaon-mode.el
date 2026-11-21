;;; lykaon-mode.el --- Emacs mode for the Lykaon language  -*- lexical-binding: t; -*-

;; Author: Lykaon
;; Keywords: languages, lykaon

;;; Commentary:
;; A small Emacs major mode for Lykaon: syntax highlighting and easy LSP.
;; Load it with:
;;   (load-file "/path/to/editors/emacs/lykaon-mode.el")
;; It also registers .lykaon to open in this mode.  With lsp-mode installed,
;; M-x lsp-mode connects to `lykaonc-cli lsp`.

;;; Code:

(defvar lykaon-keywords
  '("Say" "Set" "Ask" "If" "Otherwise" "Repeat" "For" "each" "in" "from" "to"
    "while" "forever" "Stop" "Skip" "Give" "Hand" "back" "To" "A" "has" "is"
    "Changeable" "Constant" "Note" "About" "Test" "Check" "Try" "When" "Look"
    "Be" "careful" "Do" "these" "together" "Start" "task" "Wait" "Sleep"
    "Send" "Receive" "Run" "Exit" "Read" "Write" "Append" "Replace" "Make"
    "Remove" "Move" "Duplicate" "Zip" "Unzip" "Gzip" "Save" "Fetch" "Post"
    "Respond" "Serve" "Page" "Log" "Takes" "Option" "Plot" "Predict" "Open"
    "Limit" "Requires" "Ensures" "Always" "Apply" "Combine" "Use" "Share"
    "Anything" "Tagged" "Load" "Navigate" "Show" "Play" "Hide"
    "plus" "minus" "times" "divided" "by" "whole" "and" "or" "not" "yes" "no"
    "missing" "nothing" "with" "as" "of" "where" "that" "fails" "can" "fail"
    "copy" "share" "weak" "give" "lend" "change")
  "Lykaon keywords.")

(defvar lykaon-font-lock-keywords
  `((,(regexp-opt lykaon-keywords 'words) . font-lock-keyword-face)
    ("\\<\\([0-9]+\\(\\.[0-9]+\\)?\\)\\>" . font-lock-constant-face)
    ("^[ \t]*\\(Note\\|About\\)\\b.*" . font-lock-comment-face))
  "Font-lock for Lykaon.")

;;;###autoload
(define-derived-mode lykaon-mode prog-mode "Lykaon"
  "Major mode for editing Lykaon programs."
  :syntax-table (make-syntax-table)
  (setq-local comment-start "Note ")
  (setq-local comment-end ".")
  (setq-local font-lock-defaults '(lykaon-font-lock-keywords)))

;;;###autoload
(add-to-list 'auto-mode-alist '("\\.lykaon\\'" . lykaon-mode))

(provide 'lykaon-mode)
;;; lykaon-mode.el ends here