import Paragraph, {type ParagraphConfig, type ParagraphData} from '@editorjs/paragraph';
import { API, type HTMLPasteEvent} from '@editorjs/editorjs';
import { GrammarCheckerBuilder } from '../grammar/index';

/**
 * Constructor params for the Paragraph tool.
 * Use to pass initial data and settings.
 */
export interface GParagraphParams {
  /**
   * Initial data for the paragraph
   */
  data: ParagraphData;
  /**
   * Paragraph tool configuration
   */
  config?: ParagraphConfig;
  /**
   * Editor.js API
   */
  api: API;
  /**
   * Is paragraph read-only.
   */
  readOnly: boolean;
}
export class GParagraph extends Paragraph {
  private grammarChecker: any = null;
  private mutationObserver: MutationObserver | null = null;
  private _readOnly: boolean;
  private _initialData: ParagraphData;
  
  constructor({ data, config, api, readOnly }: GParagraphParams) {
    super({ data, config: config ?? {}, api, readOnly });
    this._readOnly = readOnly;
    this._initialData = data;
  }

  /**
   * Initialize grammar checker lazily (client-side only)
   */
  private initGrammarChecker(): void {
    if (!this.grammarChecker && typeof window !== 'undefined') {
      this.grammarChecker = new GrammarCheckerBuilder().build();
    }
  }

  /**
   * Shared grammar checking logic (debounced for better performance)
   */
  private performGrammarCheck(element: HTMLElement): void {
    this.initGrammarChecker();
    if (!this.grammarChecker) return;
    
    const currentText = element.textContent || '';
    if (currentText.trim()) {
      // Use debounced checking for real-time typing
      this.grammarChecker.checkGrammarDebounced(currentText, element);
    } else {
      // Clear overlays if no text
      this.grammarChecker.clearOverlays(element);
    }
  }


  /**
   * Create Tool's view
   *
   * @returns Tool's view element
   * @private
   */
  drawView(): HTMLParagraphElement {
    const paragraph = document.createElement("p");
    const placeholder = Paragraph.DEFAULT_PLACEHOLDER;
    
    paragraph.classList.add("ce-paragraph", this.api.styles.block);
    paragraph.contentEditable = "false";
    paragraph.dataset.placeholderActive = this.api.i18n.t(placeholder);
    
    // Set initial content from data (important for block conversion)
    paragraph.innerHTML = this._initialData.text || '';
    
    if (!this._readOnly) {
      paragraph.contentEditable = "true";
      paragraph.addEventListener("keyup", this.onKeyUp);
      
      // Use MutationObserver to catch all content changes (typing, paste, drag & drop)
      this.mutationObserver = new MutationObserver((mutations) => {
        let shouldCheck = false;
        
        mutations.forEach((mutation) => {
          if (mutation.type === 'childList' || mutation.type === 'characterData') {
            shouldCheck = true;
          }
        });
        
        if (shouldCheck) {
          // Use setTimeout to ensure DOM is settled before grammar checking
          setTimeout(() => this.performGrammarCheck(paragraph), 0);
        }
      });
      
      // Start observing the paragraph for changes
      this.mutationObserver.observe(paragraph, {
        childList: true,
        subtree: true,
        characterData: true,
        characterDataOldValue: true
      });
    }
    
    // Trigger initial grammar checking if there's content (for block conversion)
    if (!this._readOnly) {
      requestAnimationFrame(() => {
        if (paragraph.textContent?.trim()) {
          this.performGrammarCheck(paragraph);
        }
      });
    }
    
    return paragraph;
  }

  /**
   * Handle paste events - check grammar after content is pasted
   * Note: MutationObserver will also catch this, but onPaste provides faster response
   */
  onPaste(event: HTMLPasteEvent): void {
    // Call parent onPaste first
    super.onPaste(event);
    
    // Use requestAnimationFrame to check grammar after paste content is rendered
    window.requestAnimationFrame(() => {
      const element = this.render();
      if (element) {
        this.performGrammarCheck(element);
      }
    });
  }


  /**
   * Override save method to ensure clean content extraction without grammar overlays
   */
  save(toolsContent: HTMLParagraphElement): ParagraphData {
    // Temporarily clear overlays to get clean content
    if (this.grammarChecker) {
      this.grammarChecker.clearOverlays(toolsContent);
    }
    
    // Get the clean content using parent's save method
    const data = super.save(toolsContent);
    
    // Restore grammar checking after save
    setTimeout(() => {
      if (toolsContent.textContent?.trim()) {
        this.performGrammarCheck(toolsContent);
      }
    }, 0);
    
    return data;
  }

  /**
   * Lifecycle hook called after block is rendered
   */
  rendered(): void {
    // Trigger grammar checking after block is fully rendered (for conversion)
    if (!this._readOnly) {
      setTimeout(() => {
        const element = this.render();
        if (element && element.textContent?.trim()) {
          this.performGrammarCheck(element);
        }
      }, 50);
    }
  }

  /**
   * Clean up resources when the tool is destroyed
   */
  destroy(): void {
    if (this.mutationObserver) {
      this.mutationObserver.disconnect();
      this.mutationObserver = null;
    }
    
    // Clean up grammar checker resources
    if (this.grammarChecker) {
      this.grammarChecker.destroy();
    }
  }

}