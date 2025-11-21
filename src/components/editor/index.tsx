import {
  $,
  component$,
  isServer,
  noSerialize,
  NoSerialize,
  useOnDocument,
  useSignal,
  useTask$,
} from "@builder.io/qwik";
import EditorJS from '@editorjs/editorjs';
import { GHeader, GParagraph } from "~/plugins";


export default component$(() => {
  const editorRef = useSignal<HTMLDivElement>();
  const isLoaded = useSignal(false);
  const editorJs = useSignal<NoSerialize<EditorJS>>();
  useOnDocument('DOMContentLoaded', $(() => isLoaded.value = true));
  
  useTask$(({ track }) => {
    track(() => isLoaded.value);
    if (isServer)  return;
    const editor = new EditorJS({
      holder: editorRef.value!,
      tools: {
        paragraph: { class: GParagraph, config: { placeholder: 'Enter a paragraph...' } },
        header: { class: GHeader as any, inlineToolbar: true, config: { placeholder: 'Enter a heading...' } },
      },
    });
    editorJs.value = noSerialize(editor);
  })

  return <div ref={editorRef} class="prose prose-lg max-w-none w-full h-[500px] border border-gray-300 rounded-md p-4"/>;
});