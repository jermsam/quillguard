import { component$ } from "@builder.io/qwik";
import { type DocumentHead, /* routeLoader$ */ } from "@builder.io/qwik-city";
import Editor from "../components/editor";
// export const useProductDetails = routeLoader$(async () => {
//   // This code runs only on the server, after every navigation
//   const res = await Promise.all([
//     fetch(`http://127.0.0.1:3000/api/info`),
//     fetch(`http://127.0.0.1:3000/api/grammar`, {
//       method: "POST",
//       headers: {
//         "Content-Type": "application/json",
//       },
//       body: JSON.stringify({
//         text: `
//         You can try out a editor that uses Harper under the hood here.
// It is rnning in your browser right now. 
// No server required!
//         `,
//       }),
//     }),
//   ]);
//   const getJson = (r: Response) => r.json();
//   const [info, grammar] = await Promise.all(res.map(getJson));
//   return {
//     info,
//     grammar,
//   };
// });

export default component$(() => {
  // In order to access the `routeLoader$` data within a Qwik Component, you need to call the hook.
  // const signal = useProductDetails(); // Readonly<Signal<Product>>
  return (
    <section class={ "flex flex-col items-center justify-center gap-4 p-4"}>
      <Editor/>
     {/*  <pre>{JSON.stringify(signal.value, null, 2)}</pre> */}
    </section>
  );
});

export const head: DocumentHead = {
  title: "Welcome to Qwik",
  meta: [
    {
      name: "description",
      content: "Qwik site description",
    },
  ],
};
