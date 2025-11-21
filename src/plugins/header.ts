import Header, { type HeaderData, type HeaderConfig } from "@editorjs/header";
import { API, type PasteEvent } from "@editorjs/editorjs";
import { GrammarCheckerBuilder } from '../grammar/index';

/**
 * @description Constructor arguments for Header
 */
interface ConstructorArgs {
  /** Previously saved data */
  data: HeaderData | object;
  /** User config for the tool */
  config?: HeaderConfig;
  /** Editor.js API */
  api: API;
  /** Read-only mode flag */
  readOnly: boolean;
}
export class GHeader extends Header {
  private grammarChecker = new GrammarCheckerBuilder().build();
  private mutationObserver: MutationObserver | null = null;
  private _readOnly: boolean;
  
  constructor({ data, config, api, readOnly }: ConstructorArgs) {
    super({ data, config: config ?? {}, api, readOnly });
    this._readOnly = readOnly;
  }

  /**
   * Shared grammar checking logic
   */
  private async performGrammarCheck(element: HTMLElement): Promise<void> {
    // Always clear existing overlays first to prevent duplicates
    this.grammarChecker.clearOverlays(element);
    
    const currentText = element.textContent || '';
    if (currentText.trim()) {
      const suggestions = await this.grammarChecker.checkGrammar(currentText);
      this.grammarChecker.createOverlays(element, currentText, suggestions);
    }
  }

  /**
   * Override getTag to add grammar checking functionality
   */
  getTag(): HTMLHeadingElement {
    const tag = super.getTag();
    
    if (!this._readOnly) {
      // Add keyup event listener for grammar checking
      tag.addEventListener("keyup", () => {
        this.performGrammarCheck(tag);
      });
      
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
          setTimeout(() => this.performGrammarCheck(tag), 0);
        }
      });
      
      // Start observing the header for changes
      this.mutationObserver.observe(tag, {
        childList: true,
        subtree: true,
        characterData: true,
        characterDataOldValue: true
      });

      // Trigger initial grammar checking if there's content (for block conversion)
      // Use a longer timeout to ensure the element is fully set up
      setTimeout(() => {
        if (tag.textContent?.trim()) {
          this.performGrammarCheck(tag);
        }
      }, 100);
    }
    
    return tag;
  }

  /**
   * Handle paste events - check grammar after content is pasted
   */
  onPaste(event: PasteEvent): void {
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
   * Override data setter to trigger grammar checking after block conversion
   */
  set data(data: HeaderData) {
    // Clear any existing overlays before changing header level
    const currentElement = this.render();
    if (currentElement) {
      this.grammarChecker.clearOverlays(currentElement);
    }
    
    // Call parent data setter first (this may replace the DOM element)
    super.data = data;
    
    // Trigger grammar checking after data is set (for block conversion)
    if (!this._readOnly && data.text && data.text.trim()) {
      // Use multiple animation frames to ensure DOM is fully updated
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const element = this.render();
          if (element && element.textContent?.trim()) {
            this.performGrammarCheck(element);
          }
        });
      });
    }
  }

  /**
   * Override data getter to maintain compatibility
   */
  get data(): HeaderData {
    return super.data;
  }

  /**
   * Override save method to ensure clean content extraction without grammar overlays
   */
  save(toolsContent: HTMLHeadingElement): HeaderData {
    // Temporarily clear overlays to get clean content
    this.grammarChecker.clearOverlays(toolsContent);
    
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
  }
}
