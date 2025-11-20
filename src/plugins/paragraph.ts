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
  private config: ParagraphConfig;
  private data: ParagraphData;
  private grammarChecker = new GrammarCheckerBuilder().build();
  private mutationObserver: MutationObserver | null = null;
  
  constructor({ data, config, api, readOnly }: GParagraphParams) {
    super({ data, config: config ?? {}, api, readOnly });
    this.config = config ?? {};
    this.data = data;
  }


  /**
   * Create Tool's view
   *
   * @returns Tool's view element
   * @private
   */
  drawView(): HTMLParagraphElement {
    const paragraph = document.createElement("p");
    const placeholder =
      this.config.placeholder || Paragraph.DEFAULT_PLACEHOLDER;
    
    paragraph.classList.add("ce-paragraph", this.api.styles.block);
    paragraph.contentEditable = "false";
    paragraph.dataset.placeholderActive = this.api.i18n.t(placeholder);
    
    if (!this.readOnly) {
      paragraph.contentEditable = "true";
      paragraph.addEventListener("keyup", this.onKeyUp);
      
      // Grammar checking function
      const checkGrammar = async () => {
        const currentText = paragraph.textContent || '';
        if (currentText.trim()) {
          const suggestions = await this.grammarChecker.checkGrammar(currentText);
          // Create overlays in completely separate DOM tree
          this.grammarChecker.createOverlays(paragraph, currentText, suggestions);
        } else {
          this.grammarChecker.clearOverlays(paragraph);
        }
      };

      // Grammar checking with completely separate overlay system
      paragraph.addEventListener("input", checkGrammar);
      
      // Use MutationObserver to catch all content changes including paste
      this.mutationObserver = new MutationObserver((mutations) => {
        let shouldCheck = false;
        
        mutations.forEach((mutation) => {
          if (mutation.type === 'childList' || mutation.type === 'characterData') {
            shouldCheck = true;
          }
        });
        
        if (shouldCheck) {
          setTimeout(checkGrammar, 0); // Use setTimeout to ensure DOM is settled
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
   * Following Editor.js pattern with requestAnimationFrame
   */
  onPaste(event: HTMLPasteEvent): void {
    // Call parent onPaste first
    super.onPaste(event);
    
    // Use requestAnimationFrame to check grammar after paste content is rendered
    window.requestAnimationFrame(() => {
      // Get the rendered element
      const element = this.render();
      if (element) {
        const currentText = element.textContent || '';
        if (currentText.trim()) {
          this.grammarChecker.checkGrammar(currentText).then(suggestions => {
            this.grammarChecker.createOverlays(element, currentText, suggestions);
          });
        } else {
          this.grammarChecker.clearOverlays(element);
        }
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