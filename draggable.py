from kivymd.app import MDApp
from kivymd.uix.boxlayout import MDBoxLayout
from kivymd.uix.label import MDLabel
from kivy.uix.widget import Widget
from kivy.properties import StringProperty, BooleanProperty
from kivy.graphics import Rectangle, Color
from kivymd.uix.toolbar import MDTopAppBar


class TaskBar(Widget):
    """A draggable task bar representing a task."""
    name = StringProperty("")
    dragged = BooleanProperty(False)

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        with self.canvas:
            Color(0.2, 0.6, 1, 1)  # Material blue color
            self.rect = Rectangle(pos=self.pos, size=self.size)
        self.bind(pos=self.update_graphics, size=self.update_graphics)

    def update_graphics(self, *args):
        self.rect.pos = self.pos
        self.rect.size = self.size

    def on_touch_down(self, touch):
        if self.collide_point(*touch.pos):
            self.dragged = True
            return True
        return super().on_touch_down(touch)

    def on_touch_move(self, touch):
        if self.dragged:
            self.center_x = touch.x
            self.center_y = touch.y
            return True
        return super().on_touch_move(touch)

    def on_touch_up(self, touch):
        if self.dragged:
            self.dragged = False
            # Check if dropped in the Gantt chart
            if self.parent and hasattr(self.parent, "on_task_drop"):
                self.parent.on_task_drop(self, touch)
            return True
        return super().on_touch_up(touch)


class GanttChart(MDBoxLayout):
    """The main Gantt chart area."""
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.orientation = "vertical"
        self.add_widget(MDBoxLayout(
            text="TIMELINE GANTT AREA", halign="center", size_hint_y=0.8, size_hint_x=0.8, theme_text_color="Primary", md_bg_color = "Amber"
        ))  # Title bar

    def on_task_drop(self, task, touch):
        """Handle task drop into the Gantt chart."""
        if self.collide_point(*touch.pos):
            # Snap to grid
            grid_size = 100
            task.x = round(touch.x / grid_size) * grid_size
            task.y = 50  # Fixed row position for tasks
            # Ensure task is added to the chart
            if task not in self.children:
                self.add_widget(task)


class TaskList(MDBoxLayout):
    """A list of draggable tasks."""
    def __init__(self, gantt_chart, **kwargs):
        super().__init__(**kwargs)
        self.orientation = "vertical"  # Stack tasks top to bottom
        self.spacing = 10
        self.padding = [10, 10, 10, 10]  # Padding for better visuals
        self.gantt_chart = gantt_chart

        # Add a title for the task list
        self.add_widget(MDLabel(
            text="TASKS LIST", halign="center", size_hint_y=None, height=40, theme_text_color="Primary"
        ))

        # Create example tasks
        self.create_task("Task 1")
        self.create_task("Task 2")

    def create_task(self, task_name):
        """Create a draggable task."""
        task = TaskBar(name=task_name, size_hint=(None, None), size=(100, 30))
        task.bind(on_touch_up=lambda instance, touch: self.on_task_drag(instance, touch))
        self.add_widget(task)

    def on_task_drag(self, task, touch):
        """Handle task dragging and dropping into the Gantt chart."""
        if task.collide_point(*touch.pos) and self.gantt_chart.collide_point(*touch.pos):
            self.remove_widget(task)
            self.gantt_chart.on_task_drop(task, touch)


class MenuBar(MDTopAppBar):
    """A horizontal menu/command bar at the top."""
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.title = "MENU COMMAND BAR"
        self.elevation = 10  # Add shadow


class ProjectManagementApp(MDApp):
    def build(self):
        # Root layout
        root = MDBoxLayout(orientation="vertical", spacing=5, padding=5)

        # Add a menu/command bar at the top
        menu_bar = MenuBar()

        # Main layout for task list and Gantt chart
        main_layout = MDBoxLayout(orientation="horizontal", spacing=10, padding=10)

        # Create Gantt chart (right side)
        gantt_chart = GanttChart(size_hint=(0.8, 1))

        # Create task list (left side)
        task_list = TaskList(gantt_chart=gantt_chart, size_hint=(0.2, 1))

        # Add task list and Gantt chart to the main layout
        main_layout.add_widget(task_list)  # Task list on the left
        main_layout.add_widget(gantt_chart)  # Gantt chart on the right

        # Add the menu bar and main layout to the root layout
        root.add_widget(menu_bar)
        root.add_widget(main_layout)

        return root


if __name__ == "__main__":
    ProjectManagementApp().run()
