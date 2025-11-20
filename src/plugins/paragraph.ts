import Paragraph, {type ParagraphConfig, type ParagraphData} from '@editorjs/paragraph';
import { API} from '@editorjs/editorjs';
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
      
      // Grammar checking with completely separate overlay system
      paragraph.addEventListener("input", async () => {
        const currentText = paragraph.textContent || '';
        if (currentText.trim()) {
          const suggestions = await this.grammarChecker.checkGrammar(currentText);
          // Create overlays in completely separate DOM tree
          this.grammarChecker.createOverlays(paragraph, currentText, suggestions);
        } else {
          this.grammarChecker.clearOverlays(paragraph);
        }
      });
    }
    
    return paragraph;
  }

}