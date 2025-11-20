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
  private grammarChecker = new GrammarCheckerBuilder().build();
  private mutationObserver: MutationObserver | null = null;
  
  constructor({ data, config, api, readOnly }: GParagraphParams) {
    super({ data, config: config ?? {}, api, readOnly });
  }

  /**
   * Shared grammar checking logic
   */
  private async performGrammarCheck(element: HTMLElement): Promise<void> {
    const currentText = element.textContent || '';
    if (currentText.trim()) {
      const suggestions = await this.grammarChecker.checkGrammar(currentText);
      this.grammarChecker.createOverlays(element, currentText, suggestions);
    } else {
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
    
    if (!this.readOnly) {
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
   * Clean up resources when the tool is destroyed
   */
  destroy(): void {
    if (this.mutationObserver) {
      this.mutationObserver.disconnect();
      this.mutationObserver = null;
    }
  }

}