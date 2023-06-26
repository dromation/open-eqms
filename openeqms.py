import sys, os, pathlib
from typing import Union
import socket
from time import time
from datetime import datetime
from os.path import dirname, join
import cv2
import numpy as np
import glob
#from root.ModelPrep import validate_params, change_koatuu, prep_params, load_regression

#IMPORT PANDAS
#import pandas as pd
#import sqlite3

#IMPORT SCIKITLEARN FOR MACHINE LEARNING
#import scikitlearn as sct

#IMPORT KIVY FOR GUI
import kivy
#from kivy.app import App
from kivymd.app import MDApp
from kivy.uix.widget import Widget
from kivymd.uix.widget import MDWidget
from kivymd.uix import *
from kivy.properties import ObjectProperty, NumericProperty, StringProperty, BooleanProperty,\
    ListProperty
from kivy.metrics import dp
from kivy.lang import Builder
#from kivymd.app import Builder
from kivy.core.window import Window
from kivy.uix.slider import Slider
from kivymd.uix.list import IRightBodyTouch, OneLineAvatarIconListItem
from kivymd.uix.boxlayout import MDBoxLayout
#from kivy.uix.screenmanager import Screen, ScreenManager
from kivymd.uix.screen import MDScreen
from kivymd.uix.screenmanager import MDScreenManager
from kivymd.uix.screenmanager import ScreenManager
from kivymd.uix.pickers import MDColorPicker
from kivymd.uix.toolbar import MDTopAppBar
from kivymd.uix.menu import MDDropdownMenu
from kivymd.uix.button import MDRaisedButton
from kivymd.uix.button import MDIconButton
from kivy.uix.button import Button
from kivymd.uix.snackbar import Snackbar
from kivymd.uix.label import MDLabel
from kivymd.uix.fitimage import FitImage
from kivy.clock import Clock
from kivymd.uix.textfield import MDTextField
from kivymd.uix.pickers  import MDColorPicker, MDDatePicker, MDTimePicker
from kivy.uix.carousel import Carousel

#DATABASE SETUP AND DEFINITION
"""
class DBE():
    conn = sqlite3.connect('dbl_container.sql')

    def submit(self):
        conn = sqlite3.connect('dbl_container.sql')
        #create cursor
        c = conn.cursor()

        #add a record

        c.execute("INSERT INTO customers VALUES (:first)",
            {
            'first': self.root.ids.word_input.text,
            })

        #add message
        self.root.ids.word_label.text = f'{self.root.ids.word_input.text} Added'

        #clear input texts
        self.root.ids.word_input.text = ''



        #commit cnages
        conn.commit()

        #close connection
        conn.close()

    def show_records(self):
        conn = sqlite3.connect('dbl_container.sql')
        #create cursor
        c = conn.cursor()

        #add record

        c.execute("SELECT * FROM customers")
        
        records = c.fetchall()
        
        word = ''
        
        #loop for records
        for record in records:
            word = f'{word}\n{record[0]}'
            self.root.ids.word_label.text = f'{word}'
        
        
        #clear input texts
        self.root.ids.word_input.text = ''



        #commit cnages
        conn.commit()

        #close connection
        conn.close()

    
"""


#STATUS WORK
#define working status: by online with team, offline by yourself
#main execution is to connect and update team packages by online status

"""
class Status(self):
    container = None
    comms = sct.
    db_container = 
    
    def __init__(self):
        
    #connection to DB
    def connect(self):
        
    
    def online(self):

    def offline(self):
primary_palette = ['Red', 'Pink', 'Purple', 'DeepPurple', 
                   'Indigo', 'Blue', 'LightBlue', 'Cyan',
                     'Teal', 'Green', 'LightGreen', 'Lime',
                       'Yellow', 'Amber', 'Orange', 'DeepOrange',
                         'Brown', 'Gray', 'BlueGray']

"""
class calendars(MDDatePicker):
    
    def on_release1(self, instance):
        print(instance)

    def app_calendar(self):
        pass
    def show_app_date(self):
        date_dialog = MDDatePicker()
        date_dialog.bind(on_save=self.date_save, on_cancel=self.date_cancel)
        date_dialog.open()

    def date_save(self, instance, value):
        print(instance, value)

    def date_cancel(self, instance):
        if instance is True: pass
        elif instance is True: print(instance)


class timers(MDTimePicker):

    def time_cancel(self, instance):
        print(instance)

    def show_app_time(self, *args):
        previous_time = datetime.strptime("", '').time()
        time_dialog = MDTimePicker()
    
        time_dialog.bind(
            time = self.get_time,
            on_save=self.time_save,
            on_cancel=self.time_cancel)
        #time_dialog.set_time(previous_time)
        time_dialog.open()

    def get_time(self, time):
        time = datetime.time
        '''
        The method returns the set time.

        :type instance: <kivymd.uix.picker.MDTimePicker object>
        :type time: <class 'datetime.time'>
        '''
        return time

    def time_save(self, instance):
        print(instance)

class generator():
    pass

class mlearn():
    pass

class HOME(MDScreen):
    pass

class APPLICATIONS(MDScreen):
    pass



class Product(MDScreen):
    pass

class Drawing(MDScreen):
    def build(self):
        menu_items = [
            {
                "viewclass": "OneLineListItem",
                "text": f"Item {i}",
                "height": dp(56),
                "on_release": lambda x=f"Item {i}": self.menu_callback(x),
             } for i in range(5)
        ]
        self.menu = MDDropdownMenu(
            items=menu_items,
            width_mult=4,
        )
    def callback(self, menu_button):
        menu_button = self.root.ids.menu_button
        self.menu.caller = menu_button
        self.menu.open()

    def menu_callback(self, text_item):
        self.menu.dismiss()
        Snackbar(text=text_item).open()
        
class Casecheck(MDScreen):
    pass


class INFO(MDScreen):
    pass

class ProdcaseApp(MDApp):
    title = 'OPEN SOURCE ENTERPRISE QUALITY MANAGEMENT SYSTEM'
    sm = ScreenManager()
    apps = APPLICATIONS()

    def build(self):
        menu_items = [
            {
                "viewclass": "OneLineListItem",
                "text": f"Item {i}",
                "height": dp(56),
                "on_release": lambda x=f"Item {i}": self.menu_callback(x),
             } for i in range(5)
        ]
        self.menu = MDDropdownMenu(
            items=menu_items,
            width_mult=4,
        )
        self.theme_cls.theme_style = "Light"
        self.theme_cls.primary_palette = "LightBlue"
        self.theme_cls.material_style = "M3"
        return Builder.load_file('openeqms.kv') 
    
    def on_save(self, instance, value):
        print(instance, value)

    def on_cancel(self, instance, value):
        print(instance, value)

    def callback(self, menu_button):
        menu_button = self.root.ids.menu_button
        self.menu.caller = menu_button
        self.menu.open()

    def menu_callback(self, text_item):
        self.menu.dismiss()
        Snackbar(text=text_item).open()

    def on_pause(self):
        return True

    def on_resume(self):
        pass

    def app_calendar(self): return calendars.show_app_date(calendars)

    def app_timer(self): return timers.show_app_time(timers)

    def open_color_picker(self):
        color_picker = MDColorPicker(size_hint=(0.45, 0.85))
        color_picker.open()
        color_picker.bind(
            on_select_color=self.on_select_color,
            on_release=self.get_selected_color,
        )

    def update_color(self, color: list) -> None:
        self.root.ids.menubar.md_bg_color = color

    def get_selected_color(
        self,
        instance_color_picker: MDColorPicker,
        type_color: str,
        selected_color: Union[list, str],
    ):
        '''Return selected color.'''

        print(f"Selected color is {selected_color}")
        self.update_color(selected_color[:-1] + [1])

    def on_select_color(self, instance_gradient_tab, color: list) -> None:
        '''Called when a gradient image is clicked.'''

    def presser(self): self.root.ids.drawings.text = "Your DRAWING is missing!"

    def koko_open(self): print("koko")
    
if __name__ == '__main__':
    ProdcaseApp().run()