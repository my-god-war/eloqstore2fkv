import "../../Toc.js";
import "../../Md.js";
import "../../Code.js";
import "../../Copy.js";
import "../../Edit.js";
import { newEl } from "../../../js/dom.js";
import cNew from "../cNew.js";
import { GIT_DOC } from "../../../conf/GIT_CDN.js";

const BLOG = "blog";
let main_el, toc_el, md_el;

const initBlog = () => {
  if (!main_el) {
    main_el = newEl("main");
    main_el.className = "doc";
    toc_el = cNew("toc");
    md_el = cNew("md");
    main_el.append(toc_el, md_el);
    toc_el.target = md_el;
    toc_el.prefix = BLOG;
    toc_el.mdUrl = (key, lang) => GIT_DOC + lang + "/blog/" + key + ".md";
  }
  return main_el;
};

export default (url) => {
  if (url === BLOG || url.startsWith(BLOG + "/")) {
    const main = initBlog(),
      sub_key = url === BLOG || url === BLOG + "/" ? "" : url.slice(BLOG.length + 1);

    return [
      Promise.resolve(main),
      async (_, code) => {
        const mod = await import(`../_/blog/${code}.js`),
          dict = mod.default;
        toc_el.li = () => dict;
        const key = sub_key || (dict.README ? "README" : Object.keys(dict)[0]);
        if (key) {
          toc_el.load(key);
        }
      }
    ];
  }
};
