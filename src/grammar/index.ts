interface GrammarSuggestion {
  type: string;
  message: string;
  offset: number;
  length: number;
  replacement?: string[];
  rule?: string;
}

interface GrammarConfig {
  // Configuration options for grammar checking
  enableSpellCheck?: boolean;
  enableGrammarCheck?: boolean;
  dialect?: 'American' | 'British' | 'Canadian';
  debounceDelay?: number; // Delay in milliseconds before checking grammar (default: 500ms)
  customColors?: {
    spelling?: string;
    grammar?: string;
    style?: string;
    punctuation?: string;
    capitalization?: string;
    formatting?: string;
  };
}

class GrammarChecker {
  private currentWrapper: HTMLElement | null = null;
  private currentText: string = '';
  private debounceTimer: number | null = null;
  private readonly debounceDelay: number;
  
  constructor(private config: GrammarConfig) {
    this.debounceDelay = config.debounceDelay ?? 300; // Default 300ms delay for better responsiveness
  }

  /**
   * Fetch grammar suggestions from backend server
   */
  async fetchGrammarSuggestions(text: string): Promise<GrammarSuggestion[]> {
    if (!text.trim()) {
      return [];
    }

    try {
      const response = await fetch('http://localhost:3000/api/grammar', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          text, 
          dialect: this.config.dialect || 'American',
          use_t5: true  // Enable T5 grammar corrections
        })
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      
      if (!data.suggestions || !Array.isArray(data.suggestions)) {
        return [];
      }

      // Convert from backend format to OverlaySuggestion format
      return data.suggestions.map((s: any) => ({
        type: s.kind || 'grammar',
        message: s.message || 'Grammar issue',
        offset: Number(s.offset) || 0,
        length: Number(s.length) || 1,
        replacement: s.replacements || [],
        rule: s.kind || 'unknown'
      }));
      
    } catch (error) {
      console.error('Grammar check failed:', error);
      return [];
    }
  }

  /**
   * Check text and get suggestions from backend (debounced)
   */
  async checkGrammarDebounced(text: string, wrapper: HTMLElement): Promise<void> {
    console.log(`[DEBUG] checkGrammarDebounced called with text: "${text}"`);
    
    // Clear any existing timer
    if (this.debounceTimer !== null) {
      clearTimeout(this.debounceTimer);
      console.log(`[DEBUG] Cleared existing timer`);
    }

    // Don't clear overlays immediately - keep existing highlights until new ones are ready
    
    // Set new timer
    this.debounceTimer = window.setTimeout(async () => {
      console.log(`[DEBUG] Debounce timer fired, checking grammar for: "${text}"`);
      
      try {
        const suggestions = await this.fetchGrammarSuggestions(text);
        
        // Log detailed information about each suggestion for debugging
        if (suggestions.length > 0) {
          console.log(`Found ${suggestions.length} grammar issues:`);
          suggestions.forEach((suggestion, index) => {
            const problemText = text.slice(suggestion.offset, suggestion.offset + suggestion.length);
            console.log(`${index + 1}. "${problemText}" at position ${suggestion.offset}-${suggestion.offset + suggestion.length}`);
            console.log(`   Type: ${suggestion.type}, Message: ${suggestion.message}`);
            if (suggestion.replacement && suggestion.replacement.length > 0) {
              console.log(`   Suggestions: ${suggestion.replacement.join(', ')}`);
            }
          });
        } else {
          console.log('No grammar issues found');
        }
        
        // Clear old overlays and create new ones
        this.clearOverlays(wrapper);
        this.createOverlays(wrapper, text, suggestions);
        
      } catch (error) {
        console.error('Grammar check failed:', error);
      }
      
      this.debounceTimer = null;
    }, this.debounceDelay);
  }

  /**
   * Check text and get suggestions from backend (immediate - for programmatic use)
   */
  async checkGrammar(text: string): Promise<GrammarSuggestion[]> {
    try {
      const suggestions = await this.fetchGrammarSuggestions(text);
      
      // Log detailed information about each suggestion for debugging
      if (suggestions.length > 0) {
        console.log(`Found ${suggestions.length} grammar issues:`);
        suggestions.forEach((suggestion, index) => {
          const problemText = text.slice(suggestion.offset, suggestion.offset + suggestion.length);
          console.log(`${index + 1}. "${problemText}" at position ${suggestion.offset}-${suggestion.offset + suggestion.length}`);
          console.log(`   Type: ${suggestion.type}, Message: ${suggestion.message}`);
          if (suggestion.replacement && suggestion.replacement.length > 0) {
            console.log(`   Suggestions: ${suggestion.replacement.join(', ')}`);
          }
        });
      } else {
        console.log('No grammar issues found');
      }
      
      return suggestions;
    } catch (error) {
      console.error('Grammar check failed:', error);
      return [];
    }
  }

  /**
   * Create positioned overlays behind problematic text - completely separate from contentEditable
   */
  createOverlays(wrapper: HTMLElement, text: string, suggestions: GrammarSuggestion[]): void {
    // Store current wrapper and text for grammar fixes
    this.currentWrapper = wrapper;
    this.currentText = text;
    // Find or create overlay layer that sits OUTSIDE the wrapper entirely
    const wrapperId = wrapper.id || `grammar-target-${Date.now()}`;
    if (!wrapper.id) wrapper.id = wrapperId;
    
    let overlayLayer = document.querySelector(`#overlay-for-${wrapperId}`) as HTMLElement;
    if (!overlayLayer) {
      overlayLayer = document.createElement('div');
      overlayLayer.id = `overlay-for-${wrapperId}`;
      overlayLayer.style.position = 'fixed'; // Fixed positioning relative to viewport
      overlayLayer.style.pointerEvents = 'none'; // Layer itself doesn't intercept events
      overlayLayer.style.zIndex = '1000'; // Above most content but below modals
      overlayLayer.style.top = '0';
      overlayLayer.style.left = '0';
      overlayLayer.style.width = '100vw';
      overlayLayer.style.height = '100vh';
      
      // Insert into document body, completely separate from contentEditable
      document.body.appendChild(overlayLayer);
    }

    // Clear existing overlays
    overlayLayer.innerHTML = '';
    
    if (suggestions.length === 0) return;

    suggestions.forEach((suggestion, index) => {
      const overlay = this.createFixedOverlay(wrapper, text, suggestion, index);
      if (overlay) {
        overlayLayer.appendChild(overlay);
      }
    });
  }

  /**
   * Create a single fixed overlay for one grammar issue
   */
  private createFixedOverlay(wrapper: HTMLElement, text: string, suggestion: GrammarSuggestion, index: number): HTMLElement | null {
    try {
      // Find the actual text node in the contentEditable element
      const textNode = this.getFirstTextNode(wrapper);
      if (!textNode || !textNode.textContent) {
        console.warn('No text node found in wrapper');
        return null;
      }

      // Ensure the suggestion offset is within bounds
      const maxOffset = textNode.textContent.length;
      const startOffset = Math.min(suggestion.offset, maxOffset);
      const endOffset = Math.min(suggestion.offset + suggestion.length, maxOffset);
      
      if (startOffset >= endOffset) {
        console.warn('Invalid offset range:', startOffset, endOffset);
        return null;
      }

      // Create range for the problematic text using the actual text node
      const range = document.createRange();
      range.setStart(textNode, startOffset);
      range.setEnd(textNode, endOffset);

      // Get the position of the problematic text relative to viewport
      const rect = range.getBoundingClientRect();

      // Create background overlay element
      const overlay = document.createElement('div');
      overlay.className = `grammar-overlay grammar-${suggestion.type}`;
      overlay.style.position = 'fixed'; // Fixed to viewport, not relative to wrapper
      overlay.style.left = `${rect.left}px`; // Direct viewport coordinates
      overlay.style.top = `${rect.top}px`;
      overlay.style.width = `${rect.width}px`;
      overlay.style.height = `${rect.height}px`;
      overlay.style.backgroundColor = this.getColorForType(suggestion.type);
      overlay.style.opacity = '0.3';
      overlay.style.pointerEvents = 'auto'; // Enable clicking
      overlay.style.userSelect = 'none'; // Critical: no selection interference
      overlay.style.zIndex = '1'; // Relative to overlay layer
      overlay.style.borderRadius = '2px';
      overlay.style.display = 'block';
      overlay.style.visibility = 'visible';
      
      // Create separate underline element positioned below the text
      const underline = document.createElement('div');
      underline.className = `grammar-underline grammar-${suggestion.type}`;
      underline.style.position = 'fixed';
      underline.style.left = `${rect.left}px`;
      underline.style.top = `${rect.bottom - 1}px`; // Just 1px below the text (closer)
      underline.style.width = `${rect.width}px`;
      underline.style.height = '2px';
      underline.style.backgroundColor = this.getColorForType(suggestion.type);
      underline.style.opacity = '1'; // Solid underline
      underline.style.pointerEvents = 'auto'; // Enable clicking
      underline.style.userSelect = 'none';
      underline.style.zIndex = '1'; // Relative to overlay layer
      
      // Create container for both elements
      const container = document.createElement('div');
      container.appendChild(overlay);
      container.appendChild(underline);
      
      // Store suggestion data
      overlay.dataset.suggestionIndex = String(index);
      overlay.dataset.suggestionType = suggestion.type;
      overlay.dataset.suggestionOffset = String(suggestion.offset);
      overlay.dataset.suggestionLength = String(suggestion.length);
      
      // Add click handler to show popup
      const clickHandler = (e: MouseEvent) => {
        console.log('Grammar overlay clicked!', suggestion);
        e.stopPropagation();
        e.preventDefault();
        this.showGrammarPopup(suggestion, rect, text.slice(suggestion.offset, suggestion.offset + suggestion.length));
      };
      
      // Add cursor pointer and click handlers
      overlay.style.cursor = 'pointer';
      overlay.addEventListener('click', clickHandler);
      console.log('Added click handler to overlay');
      
      underline.style.cursor = 'pointer';
      underline.addEventListener('click', clickHandler);
      console.log('Added click handler to underline');

      const problemText = text.slice(suggestion.offset, suggestion.offset + suggestion.length);
      console.log(`Created fixed overlay for "${problemText}" at position ${suggestion.offset}-${suggestion.offset + suggestion.length}, positioned at ${rect.left}px, ${rect.top}px (${rect.width}x${rect.height})`);

      return container;
    } catch (error) {
      console.error('Failed to create overlay:', error);
      return null;
    }
  }

  /**
   * Show grammar popup with suggestion details
   */
  private showGrammarPopup(suggestion: GrammarSuggestion, rect: DOMRect, problemText: string): void {
    // Remove any existing popup
    this.hideGrammarPopup();
    
    // Create popup element with modern design
    const popup = document.createElement('div');
    popup.id = 'grammar-popup';
    popup.className = 'grammar-popup';
    popup.style.position = 'fixed';
    popup.style.left = `${rect.left}px`;
    popup.style.top = `${rect.bottom + 8}px`; // 8px below the text
    popup.style.backgroundColor = '#ffffff';
    popup.style.border = '1px solid #e1e5e9';
    popup.style.borderRadius = '12px';
    popup.style.padding = '16px';
    popup.style.boxShadow = '0 8px 32px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(0, 0, 0, 0.08)';
    popup.style.zIndex = '10000';
    popup.style.maxWidth = '320px';
    popup.style.minWidth = '280px';
    popup.style.fontSize = '14px';
    popup.style.fontFamily = '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
    popup.style.lineHeight = '1.5';
    popup.style.backdropFilter = 'blur(8px)';
    popup.style.animation = 'fadeInUp 0.2s ease-out';
    
    // Add CSS animation keyframes
    if (!document.getElementById('grammar-popup-styles')) {
      const style = document.createElement('style');
      style.id = 'grammar-popup-styles';
      style.textContent = `
        @keyframes fadeInUp {
          from {
            opacity: 0;
            transform: translateY(8px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      `;
      document.head.appendChild(style);
    }
    
    // Create popup content with clean header
    const header = document.createElement('div');
    header.style.marginBottom = '12px';
    header.style.paddingBottom = '8px';
    header.style.borderBottom = '1px solid #f0f0f0';
    
    const title = document.createElement('div');
    title.style.fontWeight = '600';
    title.style.fontSize = '15px';
    title.style.color = '#1a1a1a';
    title.textContent = `${suggestion.type.charAt(0).toUpperCase() + suggestion.type.slice(1)} Issue`;
    
    header.appendChild(title);
    
    const message = document.createElement('div');
    message.style.marginBottom = '12px';
    message.style.color = '#4a4a4a';
    message.style.fontSize = '13px';
    message.style.lineHeight = '1.4';
    message.textContent = suggestion.message;
    
    const problemWord = document.createElement('div');
    problemWord.style.marginBottom = '12px';
    problemWord.style.padding = '8px 12px';
    problemWord.style.backgroundColor = '#f8f9fa';
    problemWord.style.borderRadius = '6px';
    problemWord.style.fontSize = '13px';
    problemWord.style.color = '#666';
    problemWord.innerHTML = `<span style="color: #999;">Problem:</span> <strong style="color: ${this.getColorForType(suggestion.type)};">"${problemText}"</strong>`;
    
    popup.appendChild(header);
    popup.appendChild(message);
    popup.appendChild(problemWord);
    
    // Add suggestions if available, or helpful advice for readability issues
    if (suggestion.replacement && suggestion.replacement.length > 0) {
      const suggestionsTitle = document.createElement('div');
      suggestionsTitle.style.fontWeight = '600';
      suggestionsTitle.style.marginBottom = '8px';
      suggestionsTitle.style.fontSize = '13px';
      suggestionsTitle.style.color = '#1a1a1a';
      suggestionsTitle.textContent = 'Suggestions:';
      popup.appendChild(suggestionsTitle);
      
      const suggestionsContainer = document.createElement('div');
      suggestionsContainer.style.display = 'flex';
      suggestionsContainer.style.flexDirection = 'column';
      suggestionsContainer.style.gap = '4px';
      
      suggestion.replacement.forEach((replacement, index) => {
        const suggestionItem = document.createElement('div');
        suggestionItem.style.padding = '8px 12px';
        suggestionItem.style.backgroundColor = '#f8f9fa';
        suggestionItem.style.border = '1px solid #e9ecef';
        suggestionItem.style.borderRadius = '8px';
        suggestionItem.style.cursor = 'pointer';
        suggestionItem.style.fontSize = '13px';
        suggestionItem.style.fontWeight = '500';
        suggestionItem.style.color = '#495057';
        suggestionItem.style.transition = 'all 0.15s ease';
        suggestionItem.style.display = 'flex';
        suggestionItem.style.alignItems = 'center';
        suggestionItem.style.justifyContent = 'space-between';
        
        const replacementText = document.createElement('span');
        replacementText.textContent = replacement;
        
        const badge = document.createElement('span');
        badge.style.fontSize = '11px';
        badge.style.color = '#6c757d';
        badge.style.backgroundColor = '#e9ecef';
        badge.style.padding = '2px 6px';
        badge.style.borderRadius = '4px';
        badge.textContent = `${index + 1}`;
        
        suggestionItem.appendChild(replacementText);
        suggestionItem.appendChild(badge);
        
        suggestionsContainer.appendChild(suggestionItem);
        
        suggestionItem.addEventListener('click', () => {
          console.log(`User clicked suggestion: "${replacement}"`);
          this.applyGrammarFix(suggestion, replacement);
          this.hideGrammarPopup();
        });
        
        suggestionItem.addEventListener('mouseenter', () => {
          suggestionItem.style.backgroundColor = '#ffffff';
          suggestionItem.style.borderColor = '#d0d7de';
          suggestionItem.style.transform = 'translateY(-1px)';
          suggestionItem.style.boxShadow = '0 4px 12px rgba(0, 0, 0, 0.1)';
        });
        
        suggestionItem.addEventListener('mouseleave', () => {
          suggestionItem.style.backgroundColor = '#f8f9fa';
          suggestionItem.style.borderColor = '#e9ecef';
          suggestionItem.style.transform = 'translateY(0)';
          suggestionItem.style.boxShadow = 'none';
        });
      });
      
      popup.appendChild(suggestionsContainer);
    } else if (suggestion.type === 'readability') {
      // For readability issues, provide helpful advice instead of specific replacements
      const adviceTitle = document.createElement('div');
      adviceTitle.style.fontWeight = '600';
      adviceTitle.style.marginBottom = '8px';
      adviceTitle.style.fontSize = '13px';
      adviceTitle.style.color = '#1a1a1a';
      adviceTitle.textContent = 'Advice:';
      popup.appendChild(adviceTitle);
      
      const adviceContainer = document.createElement('div');
      adviceContainer.style.padding = '12px';
      adviceContainer.style.backgroundColor = '#f8f9fa';
      adviceContainer.style.border = '1px solid #e9ecef';
      adviceContainer.style.borderRadius = '8px';
      adviceContainer.style.fontSize = '13px';
      adviceContainer.style.lineHeight = '1.4';
      adviceContainer.style.color = '#495057';
      
      // Provide specific advice based on the issue
      if (suggestion.message.includes('words long')) {
        adviceContainer.innerHTML = `
          <strong>Consider breaking this into shorter sentences:</strong><br>
          • Split at natural break points (conjunctions like "and", "but")<br>
          • Use periods instead of commas for major ideas<br>
          • Aim for 15-20 words per sentence for better readability
        `;
      } else {
        adviceContainer.textContent = 'Consider revising this text for better clarity and readability.';
      }
      
      popup.appendChild(adviceContainer);
    }
    
    // Add modern close button
    const closeButton = document.createElement('button');
    closeButton.innerHTML = '×';
    closeButton.style.position = 'absolute';
    closeButton.style.top = '12px';
    closeButton.style.right = '12px';
    closeButton.style.width = '24px';
    closeButton.style.height = '24px';
    closeButton.style.border = 'none';
    closeButton.style.borderRadius = '50%';
    closeButton.style.backgroundColor = '#f1f3f4';
    closeButton.style.fontSize = '16px';
    closeButton.style.cursor = 'pointer';
    closeButton.style.color = '#5f6368';
    closeButton.style.display = 'flex';
    closeButton.style.alignItems = 'center';
    closeButton.style.justifyContent = 'center';
    closeButton.style.transition = 'all 0.15s ease';
    
    closeButton.addEventListener('click', () => {
      this.hideGrammarPopup();
    });
    
    popup.appendChild(closeButton);
    
    // Add to document
    document.body.appendChild(popup);
    
    // Close popup when clicking outside
    setTimeout(() => {
      document.addEventListener('click', this.hideGrammarPopup.bind(this), { once: true });
    }, 100);
  }
  
  /**
   * Apply grammar fix by replacing problematic text with suggestion
   */
  private applyGrammarFix(suggestion: GrammarSuggestion, replacement: string): void {
    if (!this.currentWrapper) {
      console.error('No current wrapper available for grammar fix');
      return;
    }

    // Get the current text from the wrapper
    const currentText = this.currentWrapper.textContent || '';
    
    // Replace the problematic text with the suggestion
    const beforeText = currentText.slice(0, suggestion.offset);
    const afterText = currentText.slice(suggestion.offset + suggestion.length);
    const newText = beforeText + replacement + afterText;
    
    // Update the wrapper text content
    this.currentWrapper.textContent = newText;
    
    // Clear overlays since text has changed
    this.clearOverlays(this.currentWrapper);
    
    // Trigger a new grammar check with the updated text
    setTimeout(async () => {
      if (this.currentWrapper) {
        const suggestions = await this.checkGrammar(newText);
        this.createOverlays(this.currentWrapper, newText, suggestions);
      }
    }, 100);
    
    console.log(`Applied grammar fix: "${currentText.slice(suggestion.offset, suggestion.offset + suggestion.length)}" → "${replacement}"`);
  }

  /**
   * Hide grammar popup
   */
  private hideGrammarPopup(): void {
    const existingPopup = document.getElementById('grammar-popup');
    if (existingPopup) {
      existingPopup.remove();
    }
  }

  /**
   * Get the first text node from the wrapper element
   */
  private getFirstTextNode(element: HTMLElement): Text | null {
    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT,
      null
    );
    return walker.nextNode() as Text | null;
  }

  /**
   * Get background color for different grammar issue types
   */
  private getColorForType(type: string): string {
    const colors: Record<string, string> = {
      spelling: '#e74c3c',     // Red
      grammar: '#e67e22',      // Orange  
      style: '#9b59b6',        // Purple
      punctuation: '#3498db',  // Blue
      capitalization: '#f1c40f', // Yellow
      formatting: '#9b59b6',   // Purple
    };
    return colors[type] || '#95a5a6'; // Default gray
  }

  /**
   * Clear all grammar overlays from wrapper
   */
  clearOverlays(wrapper: HTMLElement): void {
    const wrapperId = wrapper.id;
    if (wrapperId) {
      const overlayLayer = document.querySelector(`#overlay-for-${wrapperId}`);
      if (overlayLayer) {
        overlayLayer.innerHTML = ''; // Clear all overlays
      }
    }
  }

  /**
   * Cleanup method - call when destroying the grammar checker instance
   */
  destroy(): void {
    // Clear any pending debounce timer
    if (this.debounceTimer !== null) {
      clearTimeout(this.debounceTimer);
      this.debounceTimer = null;
    }

    // Clear overlays if we have a current wrapper
    if (this.currentWrapper) {
      this.clearOverlays(this.currentWrapper);
      this.currentWrapper = null;
    }

    // Reset state
    this.currentText = '';
  }
}

export class GrammarCheckerBuilder {
  private config: GrammarConfig = {
    enableSpellCheck: true,
    enableGrammarCheck: true,
    dialect: 'American'
  };
  
  withSpellCheck(enabled: boolean): this {
    this.config.enableSpellCheck = enabled;
    return this;
  }
  
  withGrammarCheck(enabled: boolean): this {
    this.config.enableGrammarCheck = enabled;
    return this;
  }
  
  withDialect(dialect: 'American' | 'British' | 'Canadian'): this {
    this.config.dialect = dialect;
    return this;
  }
  
  withCustomColors(colors: GrammarConfig['customColors']): this {
    this.config.customColors = colors;
    return this;
  }
  
  withDebounceDelay(delay: number): this {
    this.config.debounceDelay = delay;
    return this;
  }
  
  build(): GrammarChecker {
    return new GrammarChecker(this.config);
  } 
}
