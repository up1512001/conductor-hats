/* The build compiles .scss to a string and hands it back as the default export.
 * See the scss-text plugin in tools/build-panel.mjs. */
declare module "*.scss" {
  const css: string;
  export default css;
}
