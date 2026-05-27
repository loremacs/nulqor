import { mount } from "./panel";

const app = document.getElementById("app");
if (app) {
  mount(app);
}

document.title = "Clock";
